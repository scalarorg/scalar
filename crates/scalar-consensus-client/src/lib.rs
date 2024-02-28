pub mod proto;

use crate::proto::{CommitedTransactions, ConsensusApiClient, ExternalTransaction};
use backon::{ExponentialBuilder, Retryable};
use std::net::SocketAddr;
use tokio::sync::mpsc::UnboundedSender;

struct ConsensusClient {
    socket_addr: SocketAddr,
    tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
}

impl ConsensusClient {
    fn new(
        socket_addr: SocketAddr,
        tx_commited_transactions: UnboundedSender<Vec<ExternalTransaction>>,
    ) -> Self {
        Self {
            socket_addr,
            tx_commited_transactions,
        }
    }
}
