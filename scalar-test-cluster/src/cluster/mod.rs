use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
mod cluster;
mod config;
pub mod faucet;
mod fullnode;
mod validator;
pub use cluster::*;
pub use config::*;
pub use fullnode::*;
use sui_types::crypto::AccountKeyPair;
pub use validator::*;
pub const NUM_VALIDATOR: usize = 4;

pub struct ClusterFactory;

impl ClusterFactory {
    pub async fn start(
        options: &LocalClusterConfig,
    ) -> Result<Box<dyn ClusterTrait + Sync + Send>> {
        Ok(match &options.env {
            Env::LocalValidator => Box::new(ValidatorCluster::start(options).await?),
            Env::LocalFullnode => Box::new(FullnodeCluster::start(options).await?),
            _ => Box::new(RemoteRunningCluster::start(options).await?),
        })
    }
}

/// Cluster Abstraction
#[async_trait]
pub trait ClusterTrait {
    async fn start(options: &LocalClusterConfig) -> Result<Self>
    where
        Self: Sized;

    fn user_key(&self) -> AccountKeyPair;
    /// Place to put config for the wallet, and any locally running services.
    fn config_directory(&self) -> &Path;
}

/// Cluster Abstraction
#[async_trait]
pub trait FullnodeClusterTrait: ClusterTrait {
    fn fullnode_url(&self) -> &str;

    fn indexer_url(&self) -> &Option<String>;

    /// Returns faucet url in a remote cluster.
    fn remote_faucet_url(&self) -> Option<&str>;

    /// Returns faucet key in a local cluster.
    fn local_faucet_key(&self) -> Option<&AccountKeyPair>;
}

// Make linter happy
#[async_trait]
impl FullnodeClusterTrait for Box<dyn FullnodeClusterTrait + Send + Sync> {
    fn fullnode_url(&self) -> &str {
        (**self).fullnode_url()
    }
    fn indexer_url(&self) -> &Option<String> {
        (**self).indexer_url()
    }

    fn remote_faucet_url(&self) -> Option<&str> {
        (**self).remote_faucet_url()
    }

    fn local_faucet_key(&self) -> Option<&AccountKeyPair> {
        (**self).local_faucet_key()
    }
}

// Make linter happy
#[async_trait]
impl ClusterTrait for Box<dyn FullnodeClusterTrait + Send + Sync> {
    async fn start(_options: &LocalClusterConfig) -> Result<Self, anyhow::Error> {
        unreachable!(
            "If we already have a boxed Cluster trait object we wouldn't have to call this function"
        );
    }

    fn user_key(&self) -> AccountKeyPair {
        (**self).user_key()
    }

    fn config_directory(&self) -> &Path {
        (**self).config_directory()
    }
}
