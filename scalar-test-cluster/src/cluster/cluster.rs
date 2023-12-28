use crate::{ClusterTrait, Env, FullnodeClusterTrait, LocalClusterConfig};
use async_trait::async_trait;
use std::path::Path;
use sui_types::crypto::{get_key_pair, AccountKeyPair};

const DEVNET_FAUCET_ADDR: &str = "https://faucet.devnet.sui.io:443";
const STAGING_FAUCET_ADDR: &str = "https://faucet.staging.sui.io:443";
const CONTINUOUS_FAUCET_ADDR: &str = "https://faucet.ci.sui.io:443";
const CONTINUOUS_NOMAD_FAUCET_ADDR: &str = "https://faucet.nomad.ci.sui.io:443";
const TESTNET_FAUCET_ADDR: &str = "https://faucet.testnet.sui.io:443";
const DEVNET_FULLNODE_ADDR: &str = "https://rpc.devnet.sui.io:443";
const STAGING_FULLNODE_ADDR: &str = "https://fullnode.staging.sui.io:443";
const CONTINUOUS_FULLNODE_ADDR: &str = "https://fullnode.ci.sui.io:443";
const CONTINUOUS_NOMAD_FULLNODE_ADDR: &str = "https://fullnode.nomad.ci.sui.io:443";
const TESTNET_FULLNODE_ADDR: &str = "https://fullnode.testnet.sui.io:443";

/// Represents an up and running cluster deployed remotely.
pub struct RemoteRunningCluster {
    fullnode_url: String,
    faucet_url: String,
    config_directory: tempfile::TempDir,
}

#[async_trait]
impl ClusterTrait for RemoteRunningCluster {
    async fn start(options: &LocalClusterConfig) -> Result<Self, anyhow::Error> {
        let (fullnode_url, faucet_url) = match options.env {
            Env::Devnet => (
                String::from(DEVNET_FULLNODE_ADDR),
                String::from(DEVNET_FAUCET_ADDR),
            ),
            Env::Staging => (
                String::from(STAGING_FULLNODE_ADDR),
                String::from(STAGING_FAUCET_ADDR),
            ),
            Env::Ci => (
                String::from(CONTINUOUS_FULLNODE_ADDR),
                String::from(CONTINUOUS_FAUCET_ADDR),
            ),
            Env::CiNomad => (
                String::from(CONTINUOUS_NOMAD_FULLNODE_ADDR),
                String::from(CONTINUOUS_NOMAD_FAUCET_ADDR),
            ),
            Env::Testnet => (
                String::from(TESTNET_FULLNODE_ADDR),
                String::from(TESTNET_FAUCET_ADDR),
            ),
            Env::CustomRemote => (
                options
                    .fullnode_address
                    .clone()
                    .expect("Expect 'fullnode_address' for Env::Custom"),
                options
                    .faucet_address
                    .clone()
                    .expect("Expect 'faucet_address' for Env::Custom"),
            ),
            _ => unreachable!("Local shouldn't use RemoteRunningCluster"),
        };

        // TODO: test connectivity before proceeding?

        Ok(Self {
            fullnode_url,
            faucet_url,
            config_directory: tempfile::tempdir()?,
        })
    }

    fn user_key(&self) -> AccountKeyPair {
        get_key_pair().1
    }

    fn config_directory(&self) -> &Path {
        self.config_directory.path()
    }
}

#[async_trait]
impl FullnodeClusterTrait for RemoteRunningCluster {
    fn fullnode_url(&self) -> &str {
        self.fullnode_url.as_str()
    }

    fn indexer_url(&self) -> &Option<String> {
        &None
    }

    fn remote_faucet_url(&self) -> Option<&str> {
        Some(&self.faucet_url)
    }

    fn local_faucet_key(&self) -> Option<&AccountKeyPair> {
        None
    }
}
