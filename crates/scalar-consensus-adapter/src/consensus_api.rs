use crate::api::{ConsensusApiOpenRpc, ConsensusApiServer, ConsensusMetrics};
use crate::types::ConsensusAddTransactionResponse;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use jsonrpsee::RpcModule;
use scalar_json_rpc::SuiRpcModule;
use scalar_json_rpc::{error::Error, with_tracing};
use scalar_types::base_types::TransactionDigest;
use std::sync::Arc;
use sui_open_rpc::Module;
use tracing::instrument;
// An implementation of the read portion of the JSON-RPC interface intended for use in
// Fullnodes.
#[derive(Clone)]
pub struct ConsensusApi {
    // pub state: Arc<dyn StateRead>,
    // pub transaction_kv_store: Arc<TransactionKeyValueStore>,
    pub metrics: Arc<ConsensusMetrics>,
}

impl ConsensusApi {
    pub fn new(metrics: Arc<ConsensusMetrics>) -> Self {
        Self { metrics }
    }
}
impl SuiRpcModule for ConsensusApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        ConsensusApiOpenRpc::module_doc()
    }
}
#[async_trait]
impl ConsensusApiServer for ConsensusApi {
    #[instrument(skip(self))]
    async fn add_transaction(
        &self,
        digest: TransactionDigest,
    ) -> RpcResult<ConsensusAddTransactionResponse> {
        with_tracing!(async move {
            let response = ConsensusAddTransactionResponse {};
            Ok(response)
        })
    }
}
