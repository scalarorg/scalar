use crate::api::ConsensusMetrics;
use crate::types::{ConsensusAddTransactionResponse, EthTransaction};
use fastcrypto::encoding::Base64;
use fastcrypto::traits::ToFromBytes;
use mysten_metrics::spawn_monitored_task;
use scalar_consensus_adapter_common::proto::{
    ConsensusApi, ConsensusTransactionIn, ConsensusTransactionOut,
};
use scalar_core::authority::AuthorityState;
use scalar_core::authority_client::NetworkAuthorityClient;
use scalar_core::transaction_orchestrator::TransactiondOrchestrator;
use scalar_json_rpc::authority_state::StateRead;
use scalar_json_rpc::error::{Error, SuiRpcInputError};
use scalar_json_rpc_types::{SuiTransactionBlock, SuiTransactionBlockResponseOptions};
use scalar_types::base_types::SuiAddress;
use scalar_types::quorum_driver_types::{
    ExecuteTransactionRequest, ExecuteTransactionRequestType, ExecuteTransactionResponse,
};
use scalar_types::signature::GenericSignature;
use scalar_types::transaction::{
    InputObjectKind, Transaction, TransactionData, TransactionDataAPI,
};
use shared_crypto::intent::Intent;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::RwLock;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Response, Status};
use tracing::{info, instrument};
// use super::{transaction_orchestrator, TransactionOrchestrator};
pub type ConsensusServiceResult<T> = Result<Response<T>, Status>;
pub type ListenerCollection = Vec<UnboundedSender<Result<ConsensusTransactionOut, Status>>>;
pub type ResponseStream =
    Pin<Box<dyn Stream<Item = Result<ConsensusTransactionOut, Status>> + Send>>;

#[derive(Clone)]
pub struct EthTransactionHandler {
    state: Arc<dyn StateRead>,
    transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
}
impl EthTransactionHandler {
    pub fn new(
        state: Arc<dyn StateRead>,
        transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
    ) -> Self {
        EthTransactionHandler {
            state,
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
    pub async fn handle_consensus_transaction(
        &self,
        transaction: ConsensusTransactionIn,
    ) -> Result<ConsensusAddTransactionResponse, Error> {
        info!(
            "gRpc service handle consensus_transaction {:?}",
            &transaction
        );
        let ConsensusTransactionIn { tx_bytes, signatures } = transaction;
        let tx_bytes = Base64::from_bytes(tx_bytes.as_slice());
        let base64_sigs = signatures.iter().map(|signature| Base64::from_bytes(signature.as_bytes())).collect::<Vec<Base64>>();

        let (opts, request_type, sender, input_objs, txn, transaction, raw_transaction) =
            self.prepare_execute_transaction_block(tx_bytes, base64_sigs, None, None)?;
        let digest = *txn.digest();
        let trans_request = ExecuteTransactionRequest {
            transaction: txn,
            request_type,
        };
        let transaction_orchestrator = self.transaction_orchestrator.clone();
        let response = spawn_monitored_task!(
            transaction_orchestrator.execute_transaction_block(trans_request)
        )
        .await?
        .map_err(Error::from)?;

        let ExecuteTransactionResponse::EffectsCert(cert) = response;
        let (effects, transaction_events, is_executed_locally) = *cert;
        // let eth_transaction = EthTransaction::new();
        // let transaction: Transaction = eth_transaction.into();
        // Scalar TODO: Prepare data then call method transaction_orchestrator.execute_transaction_block
        // let transaction_orchestrator = self.transaction_orchestrator.clone();
        // let orch_timer = self.metrics.orchestrator_latency_ms.start_timer();

        // let response = spawn_monitored_task!(transaction_orchestrator.execute_transaction_block(
        //     ExecuteTransactionRequest {
        //         transaction: txn,
        //         request_type,
        //     }
        // ))
        // .await?
        // .map_err(Error::from)?;
        // drop(orch_timer);
        // Tham khảo code của json server Server
        //
        //Không sử dụng object_changes và balance_changes ở đây, Reth sẽ handle các logic này
        Ok(ConsensusAddTransactionResponse {})
    }

    #[allow(clippy::type_complexity)]
    fn prepare_execute_transaction_block(
        &self,
        tx_bytes: Base64,
        signatures: Vec<Base64>,
        opts: Option<SuiTransactionBlockResponseOptions>,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> Result<
        (
            SuiTransactionBlockResponseOptions,
            ExecuteTransactionRequestType,
            SuiAddress,
            Vec<InputObjectKind>,
            Transaction,
            Option<SuiTransactionBlock>,
            Vec<u8>,
        ),
        SuiRpcInputError,
    > {
        let opts = opts.unwrap_or_default();
        let request_type = match (request_type, opts.require_local_execution()) {
            (Some(ExecuteTransactionRequestType::WaitForEffectsCert), true) => {
                Err(SuiRpcInputError::InvalidExecuteTransactionRequestType)?
            }
            (t, _) => t.unwrap_or_else(|| opts.default_execution_request_type()),
        };
        let tx_data: TransactionData = self.convert_bytes(tx_bytes)?;
        let sender = tx_data.sender();
        let input_objs = tx_data.input_objects().unwrap_or_default();

        let mut sigs = Vec::new();
        for sig in signatures {
            sigs.push(GenericSignature::from_bytes(&sig.to_vec()?)?);
        }
        let txn = Transaction::from_generic_sig_data(tx_data, Intent::sui_transaction(), sigs);
        let raw_transaction = if opts.show_raw_input {
            bcs::to_bytes(txn.data())?
        } else {
            vec![]
        };
        let transaction = if opts.show_input {
            let epoch_store = self.state.load_epoch_store_one_call_per_task();
            Some(SuiTransactionBlock::try_from(
                txn.data().clone(),
                epoch_store.module_cache(),
            )?)
        } else {
            None
        };
        Ok((
            opts,
            request_type,
            sender,
            input_objs,
            txn,
            transaction,
            raw_transaction,
        ))
    }
    pub fn convert_bytes<T: serde::de::DeserializeOwned>(
        &self,
        tx_bytes: Base64,
    ) -> Result<T, SuiRpcInputError> {
        let data: T = bcs::from_bytes(&tx_bytes.to_vec()?)?;
        Ok(data)
    }
}
pub struct ConsensusService {
    state: Arc<AuthorityState>,
    transaction_handler: Arc<EthTransactionHandler>,
    consensus_listeners: Arc<RwLock<ListenerCollection>>,
    metrics: Arc<ConsensusMetrics>,
}
impl ConsensusService {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
        consensus_listeners: Arc<RwLock<ListenerCollection>>,
        metrics: Arc<ConsensusMetrics>,
    ) -> Self {
        let transaction_handler = Arc::new(EthTransactionHandler::new(
            state.clone(),
            transaction_orchestrator.clone(),
        ));
        Self {
            state,
            transaction_handler,
            consensus_listeners,
            metrics,
        }
    }
    pub async fn add_consensus_listener(
        &self,
        listener: UnboundedSender<Result<ConsensusTransactionOut, Status>>,
    ) {
        let mut quard = self.consensus_listeners.write().await;
        quard.push(listener);
    }
}

#[tonic::async_trait]
impl ConsensusApi for ConsensusService {
    type InitTransactionStream = ResponseStream;

    async fn init_transaction(
        &self,
        request: tonic::Request<tonic::Streaming<ConsensusTransactionIn>>,
    ) -> ConsensusServiceResult<Self::InitTransactionStream> {
        info!("ConsensusServiceServer::init_transaction");
        let mut in_stream = request.into_inner();
        let (tx_consensus, rx_consensus) = mpsc::unbounded_channel();
        self.add_consensus_listener(tx_consensus).await;
        //let tx_eth_transaction = self.tx_eth_transaction.clone();
        let transaction_handler = self.transaction_handler.clone();
        let _handle = tokio::spawn(async move {
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Scalar Todo: Convert incomming message to EthMessage
                transaction_handler
                    .handle_consensus_transaction(transaction_in)
                    .await;
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus);

        Ok(Response::new(
            Box::pin(out_stream) as Self::InitTransactionStream
        ))
    }
}
