use crate::proto::{
    ConsensusApi, ConsensusApiServer, ConsensusTransactionIn, ConsensusTransactionOut,
};
use crate::types::EthTransaction;
use prometheus::Registry;
use std::pin::Pin;
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
#[derive(Debug)]
pub struct ConsensusService {
    tx_eth_transaction: mpsc::UnboundedSender<EthTransaction>,
}
impl ConsensusService {
    pub fn new(tx_eth_transaction: mpsc::UnboundedSender<EthTransaction>) -> Self {
        Self { tx_eth_transaction }
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
        let tx_eth_transaction = self.tx_eth_transaction.clone();
        let handle = tokio::spawn(async move {
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Todo: Convert incomming message to EthMessage
                let eth_transaction = EthTransaction::new();
                tx_eth_transaction.send(eth_transaction);
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus);

        Ok(Response::new(
            Box::pin(out_stream) as Self::SendTransactionsStream
        ))
    }
}
