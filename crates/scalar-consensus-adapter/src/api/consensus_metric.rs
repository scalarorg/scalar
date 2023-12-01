use mysten_metrics::histogram::Histogram;
use prometheus::core::{AtomicU64, GenericCounter};
use prometheus::{
    register_int_counter_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry, Registry,
};
#[derive(Clone)]
pub struct ConsensusMetrics {
    pub transaction_counter: Histogram,
    pub wait_for_finality_timeout: GenericCounter<AtomicU64>,
}

impl ConsensusMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            transaction_counter: Histogram::new_in_registry(
                "scalar_consensus_transaction_counter",
                "The input limit for transaction_counter, after applying the cap",
                registry,
            ),
            wait_for_finality_timeout: register_int_counter_with_registry!(
                "tx_orchestrator_wait_for_finality_timeout",
                "Total number of txns timing out in waiting for finality Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
        }
    }
}
