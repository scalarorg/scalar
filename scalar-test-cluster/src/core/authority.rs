use std::{pin::Pin, sync::Arc};
use sui_config::certificate_deny_config::CertificateDenyConfig;
use sui_config::node::{ExpensiveSafetyCheckConfig, OverloadThresholdConfig, StateDebugDumpConfig};
use sui_config::transaction_deny_config::TransactionDenyConfig;
use sui_types::crypto::{default_hash, AuthoritySignInfo, Signer};
use sui_types::{
    base_types::*,
    committee::Committee,
    crypto::AuthoritySignature,
    error::{SuiError, SuiResult},
    fp_ensure,
    object::{Object, ObjectRead},
    transaction::*,
    SUI_SYSTEM_ADDRESS,
};
/// a Trait object for `Signer` that is:
/// - Pin, i.e. confined to one place in memory (we don't want to copy private keys).
/// - Sync, i.e. can be safely shared between threads.
///
/// Typically instantiated with Box::pin(keypair) where keypair is a `KeyPair`
///
pub type StableSyncAuthoritySigner = Pin<Arc<dyn Signer<AuthoritySignature> + Send + Sync>>;

pub struct AuthorityState {}

impl AuthorityState {
    #[allow(clippy::disallowed_methods)] // allow unbounded_channel()
    pub async fn new(
        name: AuthorityName,
        expensive_safety_check_config: ExpensiveSafetyCheckConfig,
        transaction_deny_config: TransactionDenyConfig,
        certificate_deny_config: CertificateDenyConfig,
        indirect_objects_threshold: usize,
        debug_dump_config: StateDebugDumpConfig,
        overload_threshold_config: OverloadThresholdConfig,
    ) -> Arc<Self> {
        let state = Arc::new(AuthorityState {});
        let authority_state = Arc::downgrade(&state);
        state
    }
}
