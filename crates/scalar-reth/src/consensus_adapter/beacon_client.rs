use reth_beacon_consensus::BeaconEngineMessage;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug)]
pub struct BeaconConsensusClient {
    rx_engine: UnboundedReceiver<BeaconEngineMessage>,
}

impl BeaconConsensusClient {
    pub fn new(rx_engine: UnboundedReceiver<BeaconEngineMessage>) -> Self {
        Self { rx_engine }
    }
    pub async fn start(mut self) {
        while let Some(engine_message) = self.rx_engine.recv().await {
            println!("{:?}", &engine_message);
        }
    }
}
