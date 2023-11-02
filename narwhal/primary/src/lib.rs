// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
#![warn(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    rust_2021_compatibility
)]

mod aggregators;
mod block_remover;
pub mod block_synchronizer;
mod block_waiter;
mod certificate_fetcher;
mod certifier;
#[cfg(test)]
#[path = "tests/common.rs"]
mod common;
mod grpc_server;
mod metrics;
mod primary;
mod proposer;
mod scalar_event;
mod state_handler;
mod synchronizer;
mod tss;
mod utils;

#[cfg(test)]
#[path = "tests/certificate_tests.rs"]
mod certificate_tests;

pub use crate::{
    block_remover::BlockRemover,
    block_synchronizer::{mock::MockBlockSynchronizer, BlockHeader},
    block_waiter::{BlockWaiter, GetBlockResponse},
    grpc_server::metrics::EndpointMetrics,
    metrics::PrimaryChannelMetrics,
    primary::{Primary, CHANNEL_CAPACITY, NUM_SHUTDOWN_RECEIVERS},
};
