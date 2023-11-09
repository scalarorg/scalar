use crate::{
    ConsensusForkChoiceUpdateError, ConsensusOnNewPayloadError, EthConsensusAdapterEvent,
    EthConsensusMessage, OnForkChoiceUpdated,
};
use futures::TryFutureExt;
use reth_interfaces::RethResult;
use reth_rpc_types::{
    engine::{
        CancunPayloadFields, ForkchoiceState, ForkchoiceUpdated, PayloadAttributes, PayloadStatus,
    },
    ExecutionPayload,
};
use tokio::sync::{mpsc, mpsc::UnboundedSender, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;
pub struct EthConsensusAdapterHandler {
    pub(crate) to_engine: UnboundedSender<EthConsensusMessage>,
}
impl EthConsensusAdapterHandler {
    /// Creates a new beacon consensus engine handle.
    pub fn new(to_engine: UnboundedSender<EthConsensusMessage>) -> Self {
        Self { to_engine }
    }
    /// Sends a new payload message to the beacon consensus engine and waits for a response.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/shanghai.md#engine_newpayloadv2>
    pub async fn new_payload(
        &self,
        payload: ExecutionPayload,
        cancun_fields: Option<CancunPayloadFields>,
    ) -> Result<PayloadStatus, ConsensusOnNewPayloadError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.to_engine.send(EthConsensusMessage::NewPayload {
            payload,
            cancun_fields,
            tx,
        });
        rx.await
            .map_err(|_| ConsensusOnNewPayloadError::EngineUnavailable)?
    }

    /// Sends a forkchoice update message to the beacon consensus engine and waits for a response.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/shanghai.md#engine_forkchoiceupdatedv2>
    pub async fn fork_choice_updated(
        &self,
        state: ForkchoiceState,
        payload_attrs: Option<PayloadAttributes>,
    ) -> Result<ForkchoiceUpdated, ConsensusForkChoiceUpdateError> {
        Ok(self
            .send_fork_choice_updated(state, payload_attrs)
            .map_err(|_| ConsensusForkChoiceUpdateError::EngineUnavailable)
            .await??
            .await?)
    }

    /// Sends a forkchoice update message to the beacon consensus engine and returns the receiver to
    /// wait for a response.
    fn send_fork_choice_updated(
        &self,
        state: ForkchoiceState,
        payload_attrs: Option<PayloadAttributes>,
    ) -> oneshot::Receiver<RethResult<OnForkChoiceUpdated>> {
        let (tx, rx) = oneshot::channel();
        let _ = self.to_engine.send(EthConsensusMessage::ForkchoiceUpdated {
            state,
            payload_attrs,
            tx,
        });
        rx
    }

    /// Sends a transition configuration exchagne message to the beacon consensus engine.
    ///
    /// See also <https://github.com/ethereum/execution-apis/blob/3d627c95a4d3510a8187dd02e0250ecb4331d27e/src/engine/paris.md#engine_exchangetransitionconfigurationv1>
    pub async fn transition_configuration_exchanged(&self) {
        let _ = self
            .to_engine
            .send(EthConsensusMessage::TransitionConfigurationExchanged);
    }

    /// Creates a new [`BeaconConsensusEngineEvent`] listener stream.
    pub fn event_listener(&self) -> UnboundedReceiverStream<EthConsensusAdapterEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let _ = self.to_engine.send(EthConsensusMessage::EventListener(tx));
        UnboundedReceiverStream::new(rx)
    }
}
