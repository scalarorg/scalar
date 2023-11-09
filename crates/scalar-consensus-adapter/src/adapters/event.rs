use reth_interfaces::consensus::ForkchoiceState;
use reth_primitives::{SealedBlock, SealedHeader};
use std::{sync::Arc, time::Duration};

use super::forkchoice::ForkchoiceStatus;

/// Events emitted by [crate::ConsensusHandler].
#[derive(Clone, Debug)]
pub enum EthConsensusAdapterEvent {
    /// The fork choice state was updated.
    ForkchoiceUpdated(ForkchoiceState, ForkchoiceStatus),
    /// A block was added to the canonical chain.
    CanonicalBlockAdded(Arc<SealedBlock>),
    /// A canonical chain was committed.
    CanonicalChainCommitted(Box<SealedHeader>, Duration),
    /// A block was added to the fork chain.
    ForkBlockAdded(Arc<SealedBlock>),
}
