pub mod proto;

use crate::proto::{CommitedTransactions, ConsensusApiClient, ExternalTransaction};
use backon::{ExponentialBuilder, Retryable};
use reth_primitives::TxHash;
use std::net::SocketAddr;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::{debug, error, info};

pub struct ScalarConsensusClient {
    socket_addr: SocketAddr,
    tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
}

impl ScalarConsensusClient {
    pub fn new(
        socket_addr: SocketAddr,
        tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
    ) -> Self {
        Self {
            socket_addr,
            tx_commited_transactions,
        }
    }

    pub async fn start(
        self,
        mut rx_pending_transaction: UnboundedReceiver<TxHash>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            socket_addr,
            tx_commited_transactions,
        } = self;

        let url = format!("http://{}", socket_addr);
        let mut client = ConsensusApiClient::connect(url).await?;

        info!(
            "Connected to the grpc consensus server at {:?}",
            &socket_addr
        );

        let stream = async_stream::stream! {
            while let Some(tx_hash) = rx_pending_transaction.recv().await {
                /*
                 * 231129 TaiVV
                 * Scalar TODO: convert transaction to ConsensusTransactionIn
                 */
                let tx_bytes = tx_hash.as_slice().to_vec();
                info!("Receive a pending transaction hash, send it into narwhal consensus {:?}", &tx_bytes);
                let consensus_transaction = ExternalTransaction { namespace: NAMESPACE.to_owned(), tx_bytes };
                yield consensus_transaction;
            }
        };

        let stream = Box::pin(stream);
        let response = client.init_transaction(stream).await?;
        let mut resp_stream = response.into_inner();

        while let Some(received) = resp_stream.next().await {
            match received {
                Ok(CommitedTransactions { transactions }) => {
                    info!("Received {:?} commited transactions.", transactions.len());
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

    pub async fn safe_start(self, mut rx_pending_transaction: UnboundedReceiver<TxHash>) {
        tokio::spawn(async move {
            (move || self.start(rx_pending_transaction))
                .retry(&ExponentialBuilder::default().with_max_times(100_000))
                .notify(|err, _| error!("Scalar consensus client error: {}. Retrying...", err))
                .await
        });
    }
}
