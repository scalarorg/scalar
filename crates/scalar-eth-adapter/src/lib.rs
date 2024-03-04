use scalar_consensus_client::ScalarConsensusClient;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct ScalarEthAdapter {
    consensus_client: ScalarConsensusClient,
    tx_pending_transaction: Option<UnboundedSender<TxHash>>,
}

impl ScalarEthAdapter {
    fn new(consensus_client: ScalarConsensusClient) -> Self {
        Self {
            consensus_client,
            tx_pending_transaction: None,
        }
    }

    async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Self {
            consensus_client,
            mut tx_pending_transaction,
        } = self;

        let (new_tx_pending_transaction, rx_pending_transaction) = unbounded_channel();

        tx_pending_transaction = Some(new_tx_pending_transaction);

        consensus_client.start(rx_pending_transaction)
    }
}
