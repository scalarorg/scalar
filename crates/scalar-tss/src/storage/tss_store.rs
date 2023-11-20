use super::TSSES_CF;
use crate::types::{KeyReservation, KvValue};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use typed_store::rocks::{open_cf, DBMap, MetricConf, ReadWriteOptions};
use typed_store::{reopen, Map, TypedStoreError};

#[derive(Clone)]
pub struct TssStore {
    store: Arc<RwLock<DBMap<KeyReservation, KvValue>>>,
}

impl TssStore {
    pub fn new(event_store: DBMap<KeyReservation, KvValue>) -> Self {
        Self {
            store: Arc::new(RwLock::new(event_store)),
        }
    }

    pub fn new_for_tests() -> Self {
        let rocksdb = open_cf(
            tempfile::tempdir().unwrap(),
            None,
            MetricConf::default(),
            &[TSSES_CF],
        )
        .expect("Cannot open database");
        let map = reopen!(&rocksdb, TSSES_CF;<KeyReservation, KvValue>);
        Self::new(map)
    }

    pub async fn read(&self, id: &KeyReservation) -> Result<Option<KvValue>, TypedStoreError> {
        let dbmap = self.store.read().await;
        dbmap.get(id)
    }

    #[allow(clippy::let_and_return)]
    pub async fn write(
        &self,
        key: &KeyReservation,
        value: &KvValue,
    ) -> Result<(), TypedStoreError> {
        // fail_point!("narwhal-store-before-write");

        let result = self.store.write().await.insert(key, value);

        // fail_point!("narwhal-store-after-write");
        result
    }
    pub async fn exists(&self, key: &KeyReservation) -> Result<bool, TypedStoreError> {
        self.store
            .read()
            .await
            .get(key)
            .map(|value| value.is_some())
    }
    pub async fn remove_all(
        &self,
        keys: impl IntoIterator<Item = KeyReservation>,
    ) -> Result<(), TypedStoreError> {
        self.store.write().await.multi_remove(keys)
    }
    pub async fn remove(&self, key: &String) -> Result<(), TypedStoreError> {
        self.store.write().await.remove(key)
    }
    pub async fn reserve_key(&self, key: &String) -> Result<String, TypedStoreError> {
        let dbmap = self.store.write().await;
        match dbmap.get(key) {
            Ok(res) => {
                if res.is_none() {
                    dbmap
                        .insert(key, &Vec::default())
                        .expect("Should insert key to db");
                }
                Ok(key.clone())
            }
            Err(e) => Err(e),
        }
    }
}
