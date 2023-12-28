use crate::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    authority::AuthorityState,
    authority_server::ValidatorService,
    consensus_adapter::{ConsensusAdapter, SubmitToConsensus},
    consensus_types::{ConsensusTransactionWrapper, NsTransaction},
};
use anyhow::anyhow;
use prometheus::Registry;
use scalar_consensus_common::{CommitedTransactions, ConsensusApi, ExternalTransaction};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use tonic::{Response, Status};
use tracing::{error, info, instrument};

pub type ConsensusServiceResult<T> = Result<Response<T>, Status>;
pub type ListenerCollection = Vec<UnboundedSender<Result<CommitedTransactions, Status>>>;
pub type ResponseStream = Pin<Box<dyn Stream<Item = Result<CommitedTransactions, Status>> + Send>>;

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
// impl Into<CertifiedTransaction> for ConsensusTransactionIn {
//     fn into(self) -> CertifiedTransaction {
//         // let ConsensusTransactionIn { tx_bytes, signatures } = self;
//         // let data = SenderSignedData::new_from_sender_signature();
//         // let transaction = Transaction::new_from_data_and_signer(data, sig);
//         // transaction
//         todo!();
//     }
// }

fn from_ns_transactions(ns_transactions: Vec<NsTransaction>) -> CommitedTransactions {
    let transactions = ns_transactions
        .into_iter()
        .map(|ns_tran: NsTransaction| ns_tran.into())
        .collect();
    info!("Consensus data {:?}", &transactions);
    CommitedTransactions { transactions }
}

#[derive(Clone)]
pub struct ConsensusService {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    validator_service: ValidatorService,
    epoch_store: Arc<AuthorityPerEpochStore>,
    metrics: Arc<ConsensusServiceMetrics>,
}
impl ConsensusService {
    pub fn new(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        validator_service: ValidatorService,
        epoch_store: Arc<AuthorityPerEpochStore>,
        prometheus_registry: &Registry,
    ) -> Self {
        Self {
            state,
            consensus_adapter,
            validator_service,
            epoch_store,
            metrics: Arc::new(ConsensusServiceMetrics::new(prometheus_registry)),
        }
    }
    pub async fn handle_consensus_transaction(
        &self,
        transaction_in: ExternalTransaction,
    ) -> anyhow::Result<()> {
        info!(
            "gRpc service handle consensus_transaction {:?}",
            &transaction_in
        );
        let ns_transaction = NsTransaction::from(transaction_in);
        // let transaction_wrapper = ConsensusTransactionWrapper::Namespace(ns_transaction);
        //self.validator_service.handle_transaction_for_testing(transaction.into()).await;
        // if let Ok(cetificate_tran) = self.create_certificate_transaction(transaction) {
        //     // self.validator_service.execute_certificate_for_testing(cetificate_tran).await;
        //     let authority = &self.state.name;
        //     let consensus_transaction = ConsensusTransaction::new_certificate_message(authority, cetificate_tran);
        //     self.consensus_adapter.submit_to_consensus(&consensus_transaction, &self.epoch_store).await;

        // }
        // let tx_bytes = bcs::to_bytes(&transaction_wrapper)
        //     .map_err(|err| anyhow!("{:?}", err))
        //     .expect("Serialization should not fail.");
        self.consensus_adapter
            .submit_ns_transaction_to_consensus(&ns_transaction, &self.epoch_store)
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    // fn create_certificate_transaction(&self, transaction_in: ConsensusTransactionIn) -> SuiResult<CertifiedTransaction> {
    //     let ConsensusTransactionIn { tx_bytes, signatures } = transaction_in;
    //     let authority_name = &self.state.name;
    //     let generic_signatures = signatures.iter().map(|sig| {
    //         Signature::from_bytes(sig.as_bytes()).map_err(|err|
    //             SuiError::InvalidSignature{ error: sig.to_owned()}
    //         ).map(|sig| GenericSignature::Signature(sig)).unwrap()
    //     }).collect::<Vec<GenericSignature>>();
    //     let validator_state = self.validator_service.validator_state();
    //     //validator_state.get_checkpoint_store().get_com
    //     let committee = self.epoch_store.committee().as_ref();
    //     let tx_kind = TransactionKind::ProgrammableTransaction(());
    //     let sender : SuiAddress = SuiAddress::ZERO;
    //     let gas_payment = ();
    //     let gas_budget = 1;
    //     let gas_price = 0;
    //     let tx_data = TransactionData::new(tx_kind, sender, gas_payment, gas_budget, gas_price);
    //     /*
    //      * Scalar Todo: define new Intent for modular architechture
    //      */
    //     let intent = Intent::personal_message();
    //     //let intent = Intent::sui_app();
    //     let data = SenderSignedData::new(tx_data, intent.clone(), generic_signatures);
    //     let epoch = self.epoch_store.epoch();
    //     let secret = &*self.state.secret;
    //     let authority_sig_info = AuthoritySignInfo::new(epoch, &data, intent, authority_name.clone(), secret);
    //     AuthorityStrongQuorumSignInfo::new_from_auth_sign_infos(vec![authority_sig_info], committee).map(|sig|CertifiedTransaction::new_from_data_and_sig(data, sig))
    // }
}

#[tonic::async_trait]
impl ConsensusApi for ConsensusService {
    type InitTransactionStream = ResponseStream;

    async fn init_transaction(
        &self,
        request: tonic::Request<tonic::Streaming<ExternalTransaction>>,
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
            while let Some(ns_transactions) = rx_consensus_res.recv().await {
                // Scalar Todo: Convert consensus result (VerifiedExecutableTransaction, Option<TransactionEffectsDigest>) to CommitTransactions
                info!("Consensus output {:?}", &ns_transactions);
                //let inner_transactions = verified_consensus_transaction.into_inner();
                let transaction_out = from_ns_transactions(ns_transactions);
                tx_consensus_out.send(Ok(transaction_out));
            }
        });

        let consensus_service = self.clone();
        let _handle = tokio::spawn(async move {
            let service = consensus_service;
            while let Some(client_message) = in_stream.next().await {
                match client_message {
                    Ok(transaction_in) => {
                        let _handle_res =
                            service.handle_consensus_transaction(transaction_in).await;
                    }
                    Err(err) => {
                        error!("{:?}", err);
                    }
                }
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
