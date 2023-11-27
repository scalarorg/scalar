use scalar_types::quorum_driver_types::QuorumDriverError;
use tonic::{Response, Status};

use crate::types::EthTransaction;
type ConsensusServiceResult<T> = Result<Response<T>, Status>;
pub(crate) struct TransactionOrchestrator {
    validator_state: Arc<AuthorityState>,
    metrics: Arc<ConsensusMetrics>,
}

impl TransactionOrchestrator {
    pub fn new(validator_state: Arc<AuthorityState>, metrics: Arc<ConsensusMetrics>) -> Self {
        Self {
            validator_state,
            metrics,
        }
    }

    pub async fn handle_eth_transaction(
        &self,
        eth_transaction: EthTransaction,
    ) -> ConsensusServiceResult<()> {
        let response = Response::new(());
        let transaction = eth_transaction.into();
        let transaction = self
            .validator_state
            .verify_transaction(transaction)
            .map_err(QuorumDriverError::InvalidUserSignature)?;
        Ok(response)
    }
}
