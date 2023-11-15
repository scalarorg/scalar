use super::{
    error::{KvError, KvResult},
    store::Store,
};
use crate::storage::TssStore;
use crate::types::KeyReservation;
use crate::{
    gg20::types::Entropy,
    gg20::types::Password,
    kv_manager::error::InnerKvError,
    mnemonic::bip39_bindings::{bip39_new_w24, bip39_seed},
};
use async_trait::async_trait;
use futures_util::TryFutureExt;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;
use tofn::{
    gg20::keygen::SecretRecoveryKey,
    sdk::api::{deserialize, serialize},
};
use tracing::info;
// default key to store mnemonic
const MNEMONIC_KEY: &str = "mnemonic";

// key to store mnemonic count
const MNEMONIC_COUNT_KEY: &str = "mnemonic_count";

const MNEMONIC_PASSWORD: &str = "";
const DEFAULT_RESERVE: Vec<u8> = Vec::new();
#[derive(Clone)]
pub struct KvStore {
    pub(crate) tss_store: TssStore,
    safe_keygen: bool,
}

impl KvStore {
    pub fn new(tss_store: TssStore, safe_keygen: bool) -> Self {
        Self {
            tss_store,
            safe_keygen,
        }
    }
    pub fn tss_store(&self) -> &TssStore {
        &self.tss_store
    }
    pub fn is_safe_keygen(&self) -> bool {
        self.safe_keygen
    }
    pub async fn seed(&self) -> KvResult<SecretRecoveryKey> {
        self.get_seed(MNEMONIC_KEY).await
    }
    pub async fn init_mnemonic(&self) -> KvResult<()> {
        let key = String::from(MNEMONIC_KEY);
        let mnemonic: KvResult<Vec<u8>> = self.get(&key).await;
        if mnemonic.is_err() {
            // create a new entropy
            let new_entropy = bip39_new_w24();
            self.handle_insert(new_entropy.clone()).await?;
        };
        Ok(())
    }
    /// Get mnemonic seed under key
    pub async fn get_seed(&self, key: &str) -> KvResult<SecretRecoveryKey> {
        let key = String::from(key);
        let mnemonic = self
            .tss_store
            .read(&key)
            .await
            .map_err(|err| {
                KvError::GetErr(InnerKvError::LogicalErr(format!(
                    "get_seed {:?} error {:?}",
                    &key, &err
                )))
            })?
            .unwrap_or_default()
            .try_into()
            .map_err(KvError::GetErr)?;

        match bip39_seed(mnemonic, Password(MNEMONIC_PASSWORD.to_owned())) {
            Ok(seed) => seed.as_bytes().try_into().map_err(|err| {
                KvError::ReserveErr(InnerKvError::LogicalErr(format!(
                    "Deserialize error {:?}",
                    &err
                )))
            }),
            Err(err) => Err(KvError::ReserveErr(InnerKvError::LogicalErr(format!(
                "get_seed {:?} error {:?}",
                &key, &err
            )))),
        }
    }
    /// inserts entropy to the kv-store
    /// takes ownership of entropy to delegate zeroization.
    async fn put_entropy(&self, reservation: KeyReservation, entropy: Entropy) -> KvResult<()> {
        let value = entropy
            .try_into()
            .map_err(|_err| KvError::PutErr(InnerKvError::DeserializationErr))?;
        self.tss_store
            .write(&reservation, &value)
            .await
            .map_err(|err| {
                KvError::PutErr(InnerKvError::LogicalErr(format!(
                    "put_entropy {:?} error {:?}",
                    &reservation, &err
                )))
            })
    }
    /// inserts entropy to the kv-store
    /// takes ownership of entropy to delegate zeroization.
    pub async fn handle_insert(&self, entropy: Entropy) -> KvResult<()> {
        let (key, count) = self.get_next_key().await?;

        info!(
            "Inserting mnemonic under key '{}' with total count '{}'",
            key, count
        );

        let reservation = self.tss_store.reserve_key(&key).await.map_err(|err| {
            KvError::PutErr(InnerKvError::LogicalErr(format!(
                "reserve_key {:?} error {:?}",
                &key, &err
            )))
        })?;

        // Insert before updating the count to minimize state corruption if it fails in the middle
        self.put_entropy(reservation, entropy).await?;
        let key_mnemonic_count = String::from(MNEMONIC_COUNT_KEY);
        // If delete isn't successful, the previous mnemonic count will still allow tofnd to work
        self.tss_store
            .remove(&key_mnemonic_count)
            .await
            .map_err(|err| {
                KvError::PutErr(InnerKvError::LogicalErr(format!(
                    "Remove key {:?} error {:?}",
                    MNEMONIC_COUNT_KEY, &err
                )))
            })?;

        let count_reservation = self
            .tss_store
            .reserve_key(&key_mnemonic_count)
            .await
            .map_err(|err| {
                KvError::PutErr(InnerKvError::LogicalErr(format!(
                    "Reserve key {:?} error {:?}",
                    MNEMONIC_COUNT_KEY, &err
                )))
            })?;

        let encoded_count =
            serialize(&(count + 1)).map_err(|_| KvError::PutErr(InnerKvError::SerializationErr))?;

        // If the new count isn't written, tofnd will still work with the latest mnemonic
        self.tss_store
            .write(&count_reservation, &encoded_count)
            .await
            .map_err(|err| {
                KvError::PutErr(InnerKvError::LogicalErr(format!(
                    "Write key {:?} error {:?}",
                    &count_reservation, &err
                )))
            })
    }
    /// Get the mnemonic count in the kv store.
    pub async fn seed_count(&self) -> KvResult<u32> {
        let key_mnemonic_count = KeyReservation::from(MNEMONIC_COUNT_KEY);
        let key_mnemonic = KeyReservation::from(MNEMONIC_KEY);
        match self.tss_store.read(&key_mnemonic_count).await {
            Ok(Some(encoded_count)) => Ok(deserialize(&encoded_count)
                .ok_or(KvError::GetErr(InnerKvError::DeserializationErr))?),
            // if MNEMONIC_COUNT_KEY does not exist then mnemonic count is either 0 or 1
            Ok(None) => Ok(match self.tss_store.exists(&key_mnemonic).await {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => 0,
            }),
            Err(err) => Err(KvError::GetErr(InnerKvError::LogicalErr(format!(
                "{:?}",
                &err
            )))),
        }
    }
    /// Get the next mnemonic key id.
    async fn get_next_key(&self) -> KvResult<(String, u32)> {
        let count = self.seed_count().await?;

        let key = match count {
            0 => String::from(MNEMONIC_KEY), // latest mnemonic is preserved in the original key
            _ => std::format!("{}_{}", MNEMONIC_KEY, count), // count is 0-indexed
        };

        Ok((key, count))
    }
    pub async fn remove(&self, key: &KeyReservation) -> KvResult<()> {
        self.tss_store.remove(key).await.map_err(|err| {
            KvError::PutErr(InnerKvError::LogicalErr(format!("Remove error {:?}", err)))
        })
    }
    pub async fn reserve_key(&self, key: &KeyReservation) -> KvResult<KeyReservation> {
        if let Ok(true) = self.tss_store.exists(key).await {
            return Err(KvError::ExistsErr(InnerKvError::LogicalErr(format!(
                "kv_manager key <{}> already reserved.",
                key
            ))));
        }

        // try to insert the new key with default value
        self.tss_store
            .write(key, &DEFAULT_RESERVE)
            .map_err(|err| {
                KvError::PutErr(InnerKvError::LogicalErr(format!(
                    "insert key <{}> with default value failed {:?}.",
                    key, &err
                )))
            })
            .await
            .expect("Should insert key");

        // return key reservation
        Ok(key.to_owned())
    }
}

#[async_trait]
impl<V: 'static> Store<V> for KvStore
where
    V: Debug + Send + Sync + Serialize + DeserializeOwned,
{
    async fn get(&self, key: &KeyReservation) -> KvResult<V> {
        match self.tss_store.read(key).await {
            Ok(Some(value)) => deserialize(value.as_slice())
                .ok_or(KvError::GetErr(InnerKvError::DeserializationErr)),
            _ => Err(KvError::GetErr(InnerKvError::LogicalErr(format!(
                "key <{}> does not have a value.",
                key
            )))),
        }
    }
    async fn put(&self, key: &KeyReservation, value: &V) -> KvResult<()> {
        let bytes: Vec<u8> =
            serialize(&value).map_err(|_| KvError::PutErr(InnerKvError::SerializationErr))?;
        self.tss_store.write(key, &bytes).await.map_err(|err| {
            KvError::PutErr(InnerKvError::LogicalErr(format!("Write error {:?}", err)))
        })
    }
}
