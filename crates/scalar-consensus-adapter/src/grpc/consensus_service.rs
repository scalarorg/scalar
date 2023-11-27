use crate::api::ConsensusMetrics;
use crate::proto::{ConsensusApi, ConsensusTransactionIn, ConsensusTransactionOut};
use crate::types::EthTransaction;
use scalar_core::authority::AuthorityState;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Response, Status};
use tracing::{debug, error, info, instrument, warn};

use super::{transaction_orchestrator, TransactionOrchestrator};
type ConsensusServiceResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<ConsensusTransactionOut, Status>> + Send>>;

pub struct ConsensusService {
    state: Arc<AuthorityState>,
    metrics: Arc<ConsensusMetrics>,
    transaction_orchestrator: Arc<TransactionOrchestrator>,
}
impl ConsensusService {
    pub fn new(state: Arc<AuthorityState>, metrics: Arc<ConsensusMetrics>) -> Self {
        let transaction_orchestrator = TransactionOrchestrator::new(state.clone(), metrics.clone());
        Self {
            state,
            metrics,
            transaction_orchestrator: Arc::new(transaction_orchestrator),
        }
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
        let transaction_orchestrator = self.transaction_orchestrator.clone();
        let handle = tokio::spawn(async move {
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Todo: Convert incomming message to EthMessage
                let eth_transaction = EthTransaction::new();
                transaction_orchestrator
                    .handle_eth_transaction(eth_transaction)
                    .await;
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus);

        Ok(Response::new(
            Box::pin(out_stream) as Self::SendTransactionsStream
        ))
    }
}
