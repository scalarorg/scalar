use crate::api::ConsensusMetrics;
use crate::proto::{ConsensusApi, ConsensusTransactionIn, ConsensusTransactionOut};
use crate::types::EthTransaction;
use scalar_core::authority::AuthorityState;
use scalar_core::authority_client::NetworkAuthorityClient;
use scalar_core::transaction_orchestrator::TransactiondOrchestrator;
use scalar_types::quorum_driver_types::{ExecuteTransactionRequest, ExecuteTransactionRequestType};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Response, Status};
use tracing::{debug, error, info, instrument, warn};

// use super::{transaction_orchestrator, TransactionOrchestrator};
type ConsensusServiceResult<T> = Result<Response<T>, Status>;
type ResponseStream = Pin<Box<dyn Stream<Item = Result<ConsensusTransactionOut, Status>> + Send>>;

#[derive(Clone)]
pub struct EthTransactionHandler {
    transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
}
impl EthTransactionHandler {
    pub fn new(
        transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
    ) -> Self {
        EthTransactionHandler {
            transaction_orchestrator,
        }
    }
    /*
     * 231127 TaiVV
     * Đây là enpoint xử lý sau khi có request tới từ Reth component.
     * Transaction được đẩy vào N&B consensus thông qua transaction_orchestrator
     * https://github.com/MystenLabs/sui/blob/main/crates/sui-json-rpc/src/transaction_execution_api.rs#L272
     * Sau khi xử lý xong (Consensus Commit), transactions sẽ được gửi lại Reth thông qua channel tx_consensus
     * được tạo ra trong method send_transactions
     *
     */
    pub async fn handle_consensus_transaction(&self, transaction: ConsensusTransactionIn) {
        let eth_transaction = EthTransaction::new();
        // Scalar TODO: Prepare data then call method transaction_orchestrator.execute_transaction_block
        // Tham khảo code của json server Server
        //
        // let trans_request = ExecuteTransactionRequest {
        //     transaction: txn,
        //     Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        // };
        // self.transaction_orchestrator
        //     .execute_transaction_block(trans_request)
        //     .await;
    }
}
pub struct ConsensusService {
    state: Arc<AuthorityState>,
    transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
    transaction_handler: Arc<EthTransactionHandler>,
    rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
    metrics: Arc<ConsensusMetrics>,
}
impl ConsensusService {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
        rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
        metrics: Arc<ConsensusMetrics>,
    ) -> Self {
        let transaction_handler =
            Arc::new(EthTransactionHandler::new(transaction_orchestrator.clone()));
        Self {
            state,
            transaction_orchestrator,
            transaction_handler,
            rx_ready_certificates,
            metrics,
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
        let transaction_handler = self.transaction_handler.clone();
        let handle = tokio::spawn(async move {
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Todo: Convert incomming message to EthMessage
                transaction_handler
                    .handle_consensus_transaction(transaction_in)
                    .await;
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus);

        Ok(Response::new(
            Box::pin(out_stream) as Self::SendTransactionsStream
        ))
    }
}
