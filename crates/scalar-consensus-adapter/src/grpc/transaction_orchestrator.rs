use tonic::{Response, Status};

use crate::types::EthTransaction;
type ConsensusServiceResult<T> = Result<Response<T>, Status>;
pub(crate) struct TransactionOrchestrator {}

impl TransactionOrchestrator {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn handle_eth_transaction(
        &self,
        eth_transaction: EthTransaction,
    ) -> ConsensusServiceResult<()> {
        let response = Response::new(());
        Ok(response)
    }
}
