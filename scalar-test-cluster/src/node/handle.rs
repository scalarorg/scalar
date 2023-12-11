// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! ScalarNodeHandle wraps ScalarNode in a way suitable for access by test code.
//!
//! When starting a ScalarNode directly, in a test (as opposed to using Swarm), the node may be
//! running inside of a simulator node. It is therefore a mistake to do something like:
//!
//! ```ignore
//!     use test_utils::authority::{start_node, spawn_checkpoint_processes};
//!
//!     let node = start_node(config, registry).await;
//!     spawn_checkpoint_processes(config, &[node]).await;
//! ```
//!
//! Because this would cause the checkpointing processes to be running inside the current
//! simulator node rather than the node in which the ScalarNode is running.
//!
//! ScalarNodeHandle provides an easy way to do the right thing here:
//!
//! ```ignore
//!     let node_handle = start_node(config, registry).await;
//!     node_handle.with_async(|sui_node| async move {
//!         spawn_checkpoint_processes(config, &[sui_node]).await;
//!     });
//! ```
//!
//! Code executed inside of with or with_async will run in the context of the simulator node.
//! This allows tests to break the simulator abstraction and magically mutate or inspect state that
//! is conceptually running on a different "machine", but without producing extremely confusing
//! behavior that might result otherwise. (For instance, any network connection that is initiated
//! from a task spawned from within a with or with_async will appear to originate from the correct
//! simulator node.
//!
//! It is possible to exfiltrate state:
//!
//! ```ignore
//!    let state = node_handle.with(|sui_node| sui_node.state);
//!    // DO NOT DO THIS!
//!    do_stuff_with_state(state)
//! ```
//!
//! We can't prevent this completely, but we can at least make the right way the easy way.

use super::ScalarNode;
use crate::core::authority::AuthorityState;
use std::future::Future;
use std::sync::Arc;

/// Wrap ScalarNode to allow correct access to ScalarNode in simulator tests.
pub struct ScalarNodeHandle {
    node: Option<Arc<ScalarNode>>,
    shutdown_on_drop: bool,
}

impl ScalarNodeHandle {
    pub fn new(node: Arc<ScalarNode>) -> Self {
        Self {
            node: Some(node),
            shutdown_on_drop: false,
        }
    }

    pub fn inner(&self) -> &Arc<ScalarNode> {
        self.node.as_ref().unwrap()
    }

    pub fn with<T>(&self, cb: impl FnOnce(&ScalarNode) -> T) -> T {
        let _guard = self.guard();
        cb(self.inner())
    }

    pub fn state(&self) -> Arc<AuthorityState> {
        self.with(|sui_node| sui_node.state())
    }

    pub fn shutdown_on_drop(&mut self) {
        self.shutdown_on_drop = true;
    }
}

impl Clone for ScalarNodeHandle {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            shutdown_on_drop: false,
        }
    }
}

#[cfg(not(msim))]
impl ScalarNodeHandle {
    // Must return something to silence lints above at `let _guard = ...`
    fn guard(&self) -> u32 {
        0
    }

    pub async fn with_async<'a, F, R, T>(&'a self, cb: F) -> T
    where
        F: FnOnce(&'a ScalarNode) -> R,
        R: Future<Output = T>,
    {
        cb(self.inner()).await
    }
}

#[cfg(msim)]
impl ScalarNodeHandle {
    fn guard(&self) -> sui_simulator::runtime::NodeEnterGuard {
        self.inner().sim_state.sim_node.enter_node()
    }

    pub async fn with_async<'a, F, R, T>(&'a self, cb: F) -> T
    where
        F: FnOnce(&'a ScalarNode) -> R,
        R: Future<Output = T>,
    {
        let fut = cb(self.node.as_ref().unwrap());
        self.inner()
            .sim_state
            .sim_node
            .await_future_in_node(fut)
            .await
    }
}

#[cfg(msim)]
impl Drop for ScalarNodeHandle {
    fn drop(&mut self) {
        if self.shutdown_on_drop {
            let node_id = self.inner().sim_state.sim_node.id();
            sui_simulator::runtime::Handle::try_current().map(|h| h.delete_node(node_id));
        }
    }
}

impl From<Arc<ScalarNode>> for ScalarNodeHandle {
    fn from(node: Arc<ScalarNode>) -> Self {
        ScalarNodeHandle::new(node)
    }
}
