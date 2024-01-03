// pub mod consensus_adapter;
// pub mod consensus_handler;
pub mod consensus_listener;
pub mod consensus_manager;
pub mod consensus_service;
pub mod consensus_throughput_calculator;
pub mod consensus_types;
pub mod consensus_validator;
pub mod mysticeti_adapter;
//pub mod post_consensus_tx_reorder;
pub mod scalar_validator;
pub mod scoring_decision;
pub mod core {
    pub use sui_core::authority;
    pub use sui_core::authority::authority_tests;
    pub use sui_core::authority_server;
    pub use sui_core::checkpoints;
    pub use sui_core::epoch;
    pub mod transaction_manager {
        pub use sui_core::TransactionManager;
    }
}
pub use scalar_consensus_common::{CommitedTransactions, ConsensusApi, ExternalTransaction};
use sui_types::messages_consensus::{ConsensusTransaction, ConsensusTransactionKind};

pub(crate) fn classify(transaction: &ConsensusTransaction) -> &'static str {
    match &transaction.kind {
        ConsensusTransactionKind::UserTransaction(certificate) => {
            if certificate.contains_shared_object() {
                "shared_certificate"
            } else {
                "owned_certificate"
            }
        }
        ConsensusTransactionKind::CheckpointSignature(_) => "checkpoint_signature",
        ConsensusTransactionKind::EndOfPublish(_) => "end_of_publish",
        ConsensusTransactionKind::CapabilityNotification(_) => "capability_notification",
        ConsensusTransactionKind::NewJWKFetched(_, _, _) => "new_jwk_fetched",
        ConsensusTransactionKind::RandomnessStateUpdate(_, _) => "randomness_state_update",
    }
}
