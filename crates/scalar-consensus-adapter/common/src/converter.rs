use crate::ConsensusTransactionOut;

impl From<VerifiedExecutableTransaction> for ConsensusTransactionOut {
    fn from(verified_consensus_transaction: VerifiedExecutableTransaction) -> Self {
        let digest: &[u8] = verified_consensus_transaction.digest().as_ref();
        let digest = digest.to_vec();
        let inner_transactions = verified_consensus_transaction.into_inner();
        let (data, signature) = inner_transactions.into_data_and_sig();
        info!("Consensus data {:?}", data.inner());
        ConsensusTransactionOut {
            payload: digest.to_vec(),
        }
    }
}