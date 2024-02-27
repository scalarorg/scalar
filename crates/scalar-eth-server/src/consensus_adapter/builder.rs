use std::{net::SocketAddr, sync::Arc};

use crate::{
    consensus_adapter::NAMESPACE,
    proto::{CommitedTransactions, ConsensusApiClient, ExternalTransaction},
};
use backon::{ExponentialBuilder, Retryable};
use reth_beacon_consensus::BeaconEngineMessage;
use reth_primitives::ChainSpec;
use reth_provider::BlockReaderIdExt;
use reth_transaction_pool::TransactionPool;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio_stream::StreamExt;
use tracing::{error, info};

use super::{mode::ScalarMiningMode, storage::Storage, task::ScalarMiningTask};

/// Builder type for configuring the setup
#[derive(Debug)]
pub struct AdapterBuilder<Client, Pool: TransactionPool> {
    chain_spec: Arc<ChainSpec>,
    client: Client,
    storage: Storage,
    pool: Pool,
    to_engine: UnboundedSender<BeaconEngineMessage>,
    rx_committed_transactions: UnboundedReceiver<Vec<ExternalTransaction>>,
}

impl<Client, Pool: TransactionPool + 'static> AdapterBuilder<Client, Pool>
where
    Client: BlockReaderIdExt,
{
    /// Creates a new builder instance to configure all parts.
    pub fn new(
        chain_spec: Arc<ChainSpec>,
        client: Client,
        pool: Pool,
        to_engine: UnboundedSender<BeaconEngineMessage>,
        rx_committed_transactions: UnboundedReceiver<Vec<ExternalTransaction>>,
    ) -> Self {
        let latest_header = client
            .latest_header()
            .ok()
            .flatten()
            .unwrap_or_else(|| chain_spec.sealed_genesis_header());
        Self {
            chain_spec,
            client,
            storage: Storage::new(latest_header),
            pool,
            to_engine,
            rx_committed_transactions,
        }
    }

    /// Consumes the type and returns all components
    #[track_caller]
    pub fn build(self) -> ScalarMiningTask<Client, Pool> {
        let Self {
            chain_spec,
            client,
            storage,
            pool,
            to_engine,
            rx_committed_transactions,
        } = self;
        let mining_mode = ScalarMiningMode::narwhal(
            pool.pending_transactions_listener(),
            rx_committed_transactions,
        );

        ScalarMiningTask::new(
            chain_spec,
            client,
            mining_mode,
            to_engine,
            storage,
            pool.clone(),
        )
    }
}

pub async fn start_consensus_client<Pool>(
    socket_addr: SocketAddr,
    tx_pool: Pool,
    tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    Pool: TransactionPool + 'static,
{
    let url = format!("http://{}", socket_addr);
    let mut client = ConsensusApiClient::connect(url).await?;
    //let handler = ScalarConsensus { beacon_consensus_engine_handle };
    info!(
        "Connected to the grpc consensus server at {:?}",
        &socket_addr
    );
    let mut rx_pending_transaction = tx_pool.pending_transactions_listener();

    //let in_stream = tokio_stream::wrappers::ReceiverStream::new(transaction_rx)
    let stream = async_stream::stream! {
        while let Some(tx_hash) = rx_pending_transaction.recv().await {
            /*
             * Scalar: Send to consensus only transactions received from local rpc request,
             * Others can come from p2p network (External)
             */
            if let Some(tran) = tx_pool.get(&tx_hash) {
                if tran.is_local() {
                    let tx_bytes = tx_hash.as_slice().to_vec();
                    info!("Send a new pending transaction to the narwhal consensus {:?}, hash {:?}", &tran, &tx_bytes);
                    let consensus_transaction = ExternalTransaction { namespace: NAMESPACE.to_owned(), tx_bytes };
                    yield consensus_transaction;
                }
            }

        }
    };
    //pin_mut!(stream);
    let stream = Box::pin(stream);
    let response = client.init_transaction(stream).await?;
    let mut resp_stream = response.into_inner();

    while let Some(received) = resp_stream.next().await {
        match received {
            Ok(CommitedTransactions { transactions }) => {
                info!("Received commited transactions {:?}.", transactions.len());
                if let Err(err) = tx_commited_transactions.send(transactions) {
                    error!("{:?}", err);
                }
                //let _ = handler.handle_commited_transactions(transactions).await;
            }
            Err(err) => {
                return Err(Box::new(err));
            }
        }
    }

    Ok(())
}
