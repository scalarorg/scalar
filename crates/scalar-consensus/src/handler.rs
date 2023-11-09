pub struct ConsensusEngineHandle {
    pub(crate) to_engine: UnboundedSender<BeaconEngineMessage>,
}
impl ConsensusEngineHandle {
    /// Creates a new beacon consensus engine handle.
    pub fn new(to_engine: UnboundedSender<BeaconEngineMessage>) -> Self {
        Self { to_engine }
    }
}
