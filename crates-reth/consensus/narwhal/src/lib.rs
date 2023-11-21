//! Narwhal & Bullshark consensus implementation.

#![warn(
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    rustdoc::all
)]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
pub mod constants;
mod scalar_consensus;
pub use scalar_consensus::ScalarConsensus;
pub use scalar_consensus::ScalarConsensusHandles;
