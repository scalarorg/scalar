use super::error::KvResult;
use crate::types::KeyReservation;
use async_trait::async_trait;

#[async_trait]
pub trait Store<V> {
    async fn get(&self, key: &KeyReservation) -> KvResult<V>;
    async fn put(&self, key: &KeyReservation, value: &V) -> KvResult<()>;
    // async fn remove(&self, key: &KeyReservation) -> KvResult<()>;
    // async fn reserve_key(&self, key: &KeyReservation) -> KvResult<String>;
    // async fn init_mnemonic(&self) -> KvResult<()>;
}
