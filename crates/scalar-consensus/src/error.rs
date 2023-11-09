use reth_interfaces::RethError;
use reth_rpc_types::engine::ForkchoiceUpdateError;
use reth_stages::PipelineError;

/// The error type for the beacon consensus engine service
/// [BeaconConsensusEngine](crate::BeaconConsensusEngine)
///
/// Represents all possible error cases for the beacon consensus engine.
#[derive(Debug, thiserror::Error)]
pub enum ConsensusEngineError {
    /// Pipeline channel closed.
    #[error("pipeline channel closed")]
    PipelineChannelClosed,
    /// Pipeline error.
    #[error(transparent)]
    Pipeline(#[from] Box<PipelineError>),
    /// Pruner channel closed.
    #[error("pruner channel closed")]
    PrunerChannelClosed,
    //     /// Hook error.
    //     #[error(transparent)]
    //     Hook(#[from] EngineHookError),
    /// Common error. Wrapper around [RethError].
    #[error(transparent)]
    Common(#[from] RethError),
}

/// Represents error cases for an applied forkchoice update.
///
/// This represents all possible error cases, that must be returned as JSON RCP errors back to the
/// beacon node.
#[derive(Debug, thiserror::Error)]
pub enum ConsensusForkChoiceUpdateError {
    /// Thrown when a forkchoice update resulted in an error.
    #[error("forkchoice update error: {0}")]
    ForkchoiceUpdateError(#[from] ForkchoiceUpdateError),
    /// Internal errors, for example, error while reading from the database.
    #[error(transparent)]
    Internal(Box<RethError>),
    /// Thrown when the engine task is unavailable/stopped.
    #[error("beacon consensus engine task stopped")]
    EngineUnavailable,
}

/// Represents all error cases when handling a new payload.
///
/// This represents all possible error cases that must be returned as JSON RCP errors back to the
/// beacon node.
#[derive(Debug, thiserror::Error)]
pub enum ConsensusOnNewPayloadError {
    /// Thrown when the engine task is unavailable/stopped.
    #[error("beacon consensus engine task stopped")]
    EngineUnavailable,
    /// Thrown when a block has blob transactions, but is not after the Cancun fork.
    #[error("block has blob transactions, but is not after the Cancun fork")]
    PreCancunBlockWithBlobTransactions,
    /// An internal error occurred, not necessarily related to the payload.
    #[error(transparent)]
    Internal(Box<dyn std::error::Error + Send + Sync>),
}
