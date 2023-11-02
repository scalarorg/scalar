// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::NodeStorage;
use std::sync::Arc;
use store::rocks::ReadWriteOptions;
use store::rocks::{open_cf, DBMap, MetricConf};
use store::{reopen, Map, TypedStoreError};
use sui_macros::fail_point;
use tokio::sync::RwLock;
use types::{EventDigest, EventVerify};

#[derive(Clone)]
pub struct EventStore {
    store: Arc<RwLock<DBMap<EventDigest, EventVerify>>>,
}

impl EventStore {
    pub fn new(event_store: DBMap<EventDigest, EventVerify>) -> Self {
        Self {
            store: Arc::new(RwLock::new(event_store)),
        }
    }

    pub fn new_for_tests() -> Self {
        let rocksdb = open_cf(
            tempfile::tempdir().unwrap(),
            None,
            MetricConf::default(),
            &[NodeStorage::EVENTS_CF],
        )
        .expect("Cannot open database");
        let map = reopen!(&rocksdb, NodeStorage::EVENTS_CF;<EventDigest, EventVerify>);
        Self::new(map)
    }

    pub async fn read(&self, id: &EventDigest) -> Result<Option<EventVerify>, TypedStoreError> {
        self.store.read().await.get(id)
    }

    #[allow(clippy::let_and_return)]
    pub async fn write(&self, event: &EventVerify) -> Result<(), TypedStoreError> {
        fail_point!("narwhal-store-before-write");

        let result = self.store.write().await.insert(&event.digest(), event);

        fail_point!("narwhal-store-after-write");
        result
    }

    pub async fn remove_all(
        &self,
        keys: impl IntoIterator<Item = EventDigest>,
    ) -> Result<(), TypedStoreError> {
        self.store.write().await.multi_remove(keys)
    }
}
