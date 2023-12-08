use fastcrypto::encoding::Base64;
use fastcrypto::traits::ToFromBytes;
use mysten_metrics::spawn_monitored_task;
use prometheus::Registry;
use shared_crypto::intent::Intent;
use sui_types::crypto::AuthorityStrongQuorumSignInfo;
use sui_types::error::SuiResult;
use sui_types::transaction::{Transaction, CertifiedTransaction, SenderSignedData};
use sui_types::executable_transaction::VerifiedExecutableTransaction;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::RwLock;
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Response, Status};
use tracing::{info, instrument};

use crate::core::authority::AuthorityState;
use crate::core::authority_server::ValidatorService;
use crate::{ConsensusTransactionOut, ConsensusApi, ConsensusTransactionIn};

use super::consensus_adapter::ConsensusAdapter;
pub type ConsensusServiceResult<T> = Result<Response<T>, Status>;
pub type ListenerCollection = Vec<UnboundedSender<Result<ConsensusTransactionOut, Status>>>;
pub type ResponseStream =
    Pin<Box<dyn Stream<Item = Result<ConsensusTransactionOut, Status>> + Send>>;

    #[derive(Clone)]
pub struct ConsensusServiceMetrics {
    // pub transaction_counter: Histogram,
    // pub wait_for_finality_timeout: GenericCounter<AtomicU64>,
}

impl ConsensusServiceMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            // transaction_counter: Histogram::new_in_registry(
            //     "scalar_consensus_transaction_counter",
            //     "The input limit for transaction_counter, after applying the cap",
            //     registry,
            // ),
            // wait_for_finality_timeout: register_int_counter_with_registry!(
            //     "tx_orchestrator_wait_for_finality_timeout",
            //     "Total number of txns timing out in waiting for finality Transaction Orchestrator handles",
            //     registry,
            // )
            // .unwrap(),
        }
    }
}
impl Into<CertifiedTransaction> for ConsensusTransactionIn {
    fn into(self) -> CertifiedTransaction {
        // let ConsensusTransactionIn { tx_bytes, signatures } = self;
        // let data = SenderSignedData::new_from_sender_signature();
        // let transaction = Transaction::new_from_data_and_signer(data, sig);
        // transaction
        todo!();
    }
}
impl From<VerifiedExecutableTransaction> for ConsensusTransactionOut {
    fn from(transaction: VerifiedExecutableTransaction) -> Self {
        let digest: &[u8] = transaction.digest().as_ref();
        ConsensusTransactionOut {
            payload: digest.to_vec(),
        }
    }
}
#[derive(Clone)]
pub struct ConsensusService {
    state: Arc<AuthorityState>,
    validator_service: ValidatorService,
    metrics: Arc<ConsensusServiceMetrics>,
}
impl ConsensusService {
    pub fn new(
        state: Arc<AuthorityState>,
        validator_service: ValidatorService,
        prometheus_registry: &Registry,
    ) -> Self {
        Self {
            state,
            validator_service,
            metrics: Arc::new(ConsensusServiceMetrics::new(prometheus_registry)),
        }
    }
    pub async fn handle_consensus_transaction(
        &self,
        transaction: ConsensusTransactionIn,
    ) -> anyhow::Result<()> {
        info!(
            "gRpc service handle consensus_transaction {:?}",
            &transaction
        );
        //self.validator_service.handle_transaction_for_testing(transaction.into()).await;
        if let Ok(cetificate_tran) = self.create_certificate_transaction(transaction) {
            self.validator_service.execute_certificate_for_testing(cetificate_tran).await;
        }
        
        Ok(())
    }
    fn create_certificate_transaction(&self, transaction_in: ConsensusTransactionIn) -> SuiResult<CertifiedTransaction> {
        let ConsensusTransactionIn { tx_bytes, signatures } = transaction_in;
        let validator_state = self.validator_service.validator_state();
        validator_state.get_checkpoint_store()
        let data = SenderSignedData::new_from_sender_signature();
        AuthorityStrongQuorumSignInfo::new_from_auth_sign_infos().map(|sig|CertifiedTransaction::new_from_data_and_sig(data, sig))
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
        //Receive consensus result, convert into right format for grpc client
        let (tx_consensus_out, rx_consensus_out) = mpsc::unbounded_channel();
        let state = self.state.clone();
        //Handle consensus ouput
        let _consensus_handle_out = tokio::spawn(async move {
            let (tx_consensus_res, mut rx_consensus_res) = mpsc::unbounded_channel();
            let consensus_listeners = state.get_consensus_listeners();
            consensus_listeners.add_listener(tx_consensus_res).await;
            while let Some((transaction, effects_digest)) = rx_consensus_res.recv().await {
                // Scalar Todo: Convert consensus result (VerifiedExecutableTransaction, Option<TransactionEffectsDigest>) to ConsensusTransactionOut
                info!("Consensus output {:?}", &transaction);
                let transaction_out = ConsensusTransactionOut::from(transaction);
                tx_consensus_out.send(Ok(transaction_out));
            }
        });
       

        let consensus_service = self.clone();
        let _handle = tokio::spawn(async move {
            let service = consensus_service;
            while let Some(Ok(transaction_in)) = in_stream.next().await {
                //Scalar Todo: Convert incomming message to EthMessage
                let _handle_res = service.handle_consensus_transaction(transaction_in).await;
            }
        });
        let out_stream = UnboundedReceiverStream::new(rx_consensus_out);

        Ok(Response::new(
            Box::pin(out_stream) as Self::InitTransactionStream
        ))
    }
}


// #[derive(Clone)]
// pub struct EthTransactionHandler {
//     state: Arc<dyn StateRead>,
//     transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
// }
// impl EthTransactionHandler {
//     pub fn new(
//         state: Arc<dyn StateRead>,
//         transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
//     ) -> Self {
//         EthTransactionHandler {
//             state,
//             transaction_orchestrator,
//         }
//     }
//     /*
//      * 231127 TaiVV
//      * Đây là enpoint xử lý sau khi có request tới từ Reth component.
//      * Transaction được đẩy vào N&B consensus thông qua transaction_orchestrator
//      * https://github.com/MystenLabs/sui/blob/main/crates/sui-json-rpc/src/transaction_execution_api.rs#L272
//      * Sau khi xử lý xong (Consensus Commit), transactions sẽ được gửi lại Reth thông qua channel tx_consensus
//      * được tạo ra trong method send_transactions
//      *
//      */
//     pub async fn handle_consensus_transaction(
//         &self,
//         transaction: ConsensusTransactionIn,
//     ) -> Result<ConsensusAddTransactionResponse, Error> {
//         info!(
//             "gRpc service handle consensus_transaction {:?}",
//             &transaction
//         );
//         let ConsensusTransactionIn { tx_hash, signature } = transaction;
//         let tx_bytes = Base64::from_bytes(tx_hash.as_slice());
//         let mut base64_sigs = vec![];
//         let base64 = Base64::from_bytes(signature.as_slice());
//         base64_sigs.push(base64);

//         let (opts, request_type, sender, input_objs, txn, transaction, raw_transaction) =
//             self.prepare_execute_transaction_block(tx_bytes, base64_sigs, None, None)?;
//         let digest = *txn.digest();
//         let trans_request = ExecuteTransactionRequest {
//             transaction: txn,
//             request_type,
//         };
//         let transaction_orchestrator = self.transaction_orchestrator.clone();
//         let response = spawn_monitored_task!(
//             transaction_orchestrator.execute_transaction_block(trans_request)
//         )
//         .await?
//         .map_err(Error::from)?;

//         let ExecuteTransactionResponse::EffectsCert(cert) = response;
//         let (effects, transaction_events, is_executed_locally) = *cert;
//         // let eth_transaction = EthTransaction::new();
//         // let transaction: Transaction = eth_transaction.into();
//         // Scalar TODO: Prepare data then call method transaction_orchestrator.execute_transaction_block
//         // let transaction_orchestrator = self.transaction_orchestrator.clone();
//         // let orch_timer = self.metrics.orchestrator_latency_ms.start_timer();

//         // let response = spawn_monitored_task!(transaction_orchestrator.execute_transaction_block(
//         //     ExecuteTransactionRequest {
//         //         transaction: txn,
//         //         request_type,
//         //     }
//         // ))
//         // .await?
//         // .map_err(Error::from)?;
//         // drop(orch_timer);
//         // Tham khảo code của json server Server
//         //
//         //Không sử dụng object_changes và balance_changes ở đây, Reth sẽ handle các logic này
//         Ok(ConsensusAddTransactionResponse {})
//     }

//     #[allow(clippy::type_complexity)]
//     fn prepare_execute_transaction_block(
//         &self,
//         tx_bytes: Base64,
//         signatures: Vec<Base64>,
//         opts: Option<SuiTransactionBlockResponseOptions>,
//         request_type: Option<ExecuteTransactionRequestType>,
//     ) -> Result<
//         (
//             SuiTransactionBlockResponseOptions,
//             ExecuteTransactionRequestType,
//             SuiAddress,
//             Vec<InputObjectKind>,
//             Transaction,
//             Option<SuiTransactionBlock>,
//             Vec<u8>,
//         ),
//         SuiRpcInputError,
//     > {
//         let opts = opts.unwrap_or_default();
//         let request_type = match (request_type, opts.require_local_execution()) {
//             (Some(ExecuteTransactionRequestType::WaitForEffectsCert), true) => {
//                 Err(SuiRpcInputError::InvalidExecuteTransactionRequestType)?
//             }
//             (t, _) => t.unwrap_or_else(|| opts.default_execution_request_type()),
//         };
//         let tx_data: TransactionData = self.convert_bytes(tx_bytes)?;
//         let sender = tx_data.sender();
//         let input_objs = tx_data.input_objects().unwrap_or_default();

//         let mut sigs = Vec::new();
//         for sig in signatures {
//             sigs.push(GenericSignature::from_bytes(&sig.to_vec()?)?);
//         }
//         let txn = Transaction::from_generic_sig_data(tx_data, Intent::sui_transaction(), sigs);
//         let raw_transaction = if opts.show_raw_input {
//             bcs::to_bytes(txn.data())?
//         } else {
//             vec![]
//         };
//         let transaction = if opts.show_input {
//             let epoch_store = self.state.load_epoch_store_one_call_per_task();
//             Some(SuiTransactionBlock::try_from(
//                 txn.data().clone(),
//                 epoch_store.module_cache(),
//             )?)
//         } else {
//             None
//         };
//         Ok((
//             opts,
//             request_type,
//             sender,
//             input_objs,
//             txn,
//             transaction,
//             raw_transaction,
//         ))
//     }
//     pub fn convert_bytes<T: serde::de::DeserializeOwned>(
//         &self,
//         tx_bytes: Base64,
//     ) -> Result<T, SuiRpcInputError> {
//         let data: T = bcs::from_bytes(&tx_bytes.to_vec()?)?;
//         Ok(data)
//     }
// }
