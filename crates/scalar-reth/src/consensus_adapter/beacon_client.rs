use jsonrpsee::http_client::HttpClient;
use reth_beacon_consensus::BeaconEngineMessage;
use reth_rpc_api::clients::EngineApiClient;
use reth_rpc_builder::auth::AuthServerHandle;
use reth_rpc_types::engine::{ForkchoiceUpdated, PayloadStatus};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, info};
///
/// Beacon Consensus client receives committed transactions then send them to ConsensusEngineApi
///
#[derive(Debug)]
pub struct BeaconConsensusClient {
    auth_server_handle: AuthServerHandle,
    auth_server_client: HttpClient,
    rx_engine: UnboundedReceiver<BeaconEngineMessage>,
}

impl BeaconConsensusClient {
    ///
    /// Create client with a channel receiver
    ///
    pub fn new(
        auth_server_handle: AuthServerHandle,
        rx_engine: UnboundedReceiver<BeaconEngineMessage>,
    ) -> Self {
        let auth_server_client = auth_server_handle.http_client();
        Self {
            auth_server_handle,
            auth_server_client,
            rx_engine,
        }
    }
    ///
    pub async fn start(mut self) {
        info!("Start BeaconConsensusClient");
        while let Some(engine_message) = self.rx_engine.recv().await {
            println!("Receiverd BeaconEngine {:?}", &engine_message);
            let _res = self.handle_consensus_engine_message(engine_message).await;
        }
        info!("Stop BeaconConsensusClient");
    }
}

impl BeaconConsensusClient {
    pub async fn handle_consensus_engine_message(
        &self,
        engine_message: BeaconEngineMessage,
    ) -> eyre::Result<()> {
        match engine_message {
            BeaconEngineMessage::NewPayload {
                payload,
                cancun_fields,
                tx,
            } => todo!(),
            BeaconEngineMessage::ForkchoiceUpdated {
                state,
                payload_attrs,
                tx,
            } => {
                let api_res = EngineApiClient::fork_choice_updated_v2(
                    &self.auth_server_client,
                    state,
                    payload_attrs,
                )
                .await;
                debug!("Fork_choice_update_v2 result {:?}", &api_res);
                let res = api_res.map(
                    |ForkchoiceUpdated {
                         payload_status,
                         payload_id,
                     }| {
                        let PayloadStatus {
                            status,
                            latest_valid_hash,
                        } = payload_status;
                        match status {
                            reth_rpc_types::engine::PayloadStatusEnum::Valid => {
                                OnForkChoiceUpdated::new();
                            }
                            reth_rpc_types::engine::PayloadStatusEnum::Invalid {
                                validation_error,
                            } => todo!(),
                            reth_rpc_types::engine::PayloadStatusEnum::Syncing => todo!(),
                            reth_rpc_types::engine::PayloadStatusEnum::Accepted => todo!(),
                        }
                    },
                );
                tx.send(res);
            }
            BeaconEngineMessage::TransitionConfigurationExchanged => todo!(),
            BeaconEngineMessage::EventListener(_) => todo!(),
        }
        Ok(())
    }
}
