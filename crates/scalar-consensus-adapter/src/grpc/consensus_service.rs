use crate::api::ConsensusMetrics;
use crate::proto::{
    ConsensusApi, ConsensusApiServer, ConsensusTransactionIn, ConsensusTransactionOut,
};
use crate::types::EthTransaction;
use prometheus::Registry;
use scalar_core::authority::AuthorityState;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::{
    wrappers::{ReceiverStream, UnboundedReceiverStream},
    Stream, StreamExt,
};
use tonic::{Response, Status};
use tracing::{debug, error, info, instrument, warn};
type ConsensusServiceResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<ConsensusTransactionOut, Status>> + Send>>;

pub struct ConsensusService {
    state: Arc<AuthorityState>,
    metrics: Arc<ConsensusMetrics>,
}
impl ConsensusService {
    pub fn new(state: Arc<AuthorityState>, metrics: Arc<ConsensusMetrics>) -> Self {
        Self { state, metrics }
    }
}

impl ConsensusService {
    async fn handle_eth_transaction(
        &self,
        eth_transaction: EthTransaction,
    ) -> ConsensusServiceResult<()> {
        let response = Response::new(());
        Ok(response)
    }
}
#[tonic::async_trait]
impl ConsensusApi for ConsensusService {
    type SendTransactionsStream = ResponseStream;

    async fn send_transactions(
        &self,
        request: tonic::Request<tonic::Streaming<ConsensusTransactionIn>>,
    ) -> ConsensusServiceResult<Self::SendTransactionsStream> {
        info!("ConsensusServiceServer::send_transactions");
        let mut in_stream = request.into_inner();
        let (tx_consensus, rx_consensus) = mpsc::unbounded_channel();
        //let tx_eth_transaction = self.tx_eth_transaction.clone();
        let handle = tokio::spawn(async move {
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Todo: Convert incomming message to EthMessage
                let eth_transaction = EthTransaction::new();
                self.handle_eth_transaction(eth_transaction);
                //tx_eth_transaction.send(eth_transaction);
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus);

        Ok(Response::new(
            Box::pin(out_stream) as Self::SendTransactionsStream
        ))
    }
}
