use mysten_metrics::histogram::Histogram;
#[derive(Clone)]
pub struct ConsensusMetrics {
    pub transaction_counter: Histogram,
}

impl ConsensusMetrics {
    pub fn new(registry: &prometheus::Registry) -> Self {
        Self {
            transaction_counter: Histogram::new_in_registry(
                "scalar_consensus_transaction_counter",
                "The input limit for transaction_counter, after applying the cap",
                registry,
            ),
        }
    }
}
