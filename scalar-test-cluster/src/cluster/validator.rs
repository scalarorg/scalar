use crate::ClusterTrait;
use crate::LocalClusterConfig;
use crate::NUM_VALIDATOR;
use anyhow::Result;
use async_trait::async_trait;
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use scalar_node::SuiNodeHandle;
use scalar_swarm::validator::{ValidatorSwarm, ValidatorSwarmBuilder};
use scalar_swarm_config::{
    genesis_config::GenesisConfig, network_config::NetworkConfig,
    network_config_builder::ProtocolVersionsConfig,
};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;
use std::{net::SocketAddr, path::Path};
use sui_config::{
    node::DBCheckpointConfig, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
    SUI_NETWORK_CONFIG,
};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::sui_client_config::SuiClientConfig;
use sui_sdk::SuiClient;
use sui_sdk::SuiClientBuilder;
use sui_types::{
    crypto::{get_key_pair, AccountKeyPair, KeypairTraits, SuiKeyPair},
    object::Object,
};
use tracing::{error, info};

pub struct ValidatorNodeHandle {
    pub sui_node: SuiNodeHandle,
    pub sui_client: SuiClient,
    pub rpc_client: HttpClient,
    pub consensus_url: String,
}

impl ValidatorNodeHandle {
    pub async fn new(sui_node: SuiNodeHandle, consensus_address: SocketAddr) -> Self {
        let consensus_url = format!("http://{}", consensus_address);
        let rpc_client = HttpClientBuilder::default().build(&consensus_url).unwrap();
        let sui_client = SuiClientBuilder::default()
            .build(&consensus_url)
            .await
            .unwrap();

        Self {
            sui_node,
            sui_client,
            rpc_client,
            consensus_url,
        }
    }
}

pub struct ValidatorCluster {
    pub swarm: ValidatorSwarm,
    config_directory: tempfile::TempDir,
}

impl ValidatorCluster {
    #[allow(unused)]
    pub fn swarm(&self) -> &ValidatorSwarm {
        &self.swarm
    }
}

#[async_trait]
impl ClusterTrait for ValidatorCluster {
    async fn start(options: &LocalClusterConfig) -> Result<Self> {
        // TODO: options should contain port instead of address
        let fullnode_port = options.fullnode_address.as_ref().map(|addr| {
            addr.parse::<SocketAddr>()
                .expect("Unable to parse fullnode address")
                .port()
        });

        let mut cluster_builder = ValidatorClusterBuilder::new(); //.enable_fullnode_events();
        if let Some(size) = options.cluster_size.as_ref() {
            cluster_builder = cluster_builder.with_cluster_size(size.clone());
        }
        if let Some(host) = options.consensus_rpc_host.as_ref() {
            cluster_builder = cluster_builder.with_consensus_rpc_host(host.clone());
        }
        if let Some(port) = options.consensus_rpc_port.as_ref() {
            cluster_builder = cluster_builder.with_consensus_rpc_port(port.clone());
        }
        // Check if we already have a config directory that is passed
        if let Some(config_dir) = options.config_dir.clone() {
            assert!(options.epoch_duration_ms.is_none());
            // Load the config of the Sui authority.
            let network_config_path = config_dir.join(SUI_NETWORK_CONFIG);
            match PersistedConfig::read(&network_config_path).map_err(|err| {
                err.context(format!(
                    "Cannot open Scalar network config file at {:?}",
                    network_config_path
                ))
            }) {
                Ok(network_config) => {
                    cluster_builder = cluster_builder.set_network_config(network_config);
                }
                Err(err) => {
                    error!("{:?}", err);
                    // Let the faucet account hold 1000 gas objects on genesis
                    let genesis_config = GenesisConfig::custom_genesis(1, 100);
                    // Custom genesis should be build here where we add the extra accounts
                    cluster_builder = cluster_builder.set_genesis_config(genesis_config);

                    if let Some(epoch_duration_ms) = options.epoch_duration_ms {
                        cluster_builder = cluster_builder.with_epoch_duration_ms(epoch_duration_ms);
                    }
                }
            }
            cluster_builder = cluster_builder.with_config_dir(config_dir);
        } else {
            // Let the faucet account hold 1000 gas objects on genesis
            let genesis_config = GenesisConfig::custom_genesis(1, 100);
            // Custom genesis should be build here where we add the extra accounts
            cluster_builder = cluster_builder.set_genesis_config(genesis_config);

            if let Some(epoch_duration_ms) = options.epoch_duration_ms {
                cluster_builder = cluster_builder.with_epoch_duration_ms(epoch_duration_ms);
            }
        }

        let mut validator_cluster = cluster_builder.build().await?;

        // Let nodes connect to one another
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // TODO: test connectivity before proceeding?
        Ok(validator_cluster)
    }

    fn user_key(&self) -> AccountKeyPair {
        get_key_pair().1
    }

    fn config_directory(&self) -> &Path {
        self.config_directory.path()
    }
}

impl ValidatorCluster {
    fn consensus_urls(&self) -> Vec<String> {
        self.swarm()
            .validator_nodes()
            .map(|node| node.config.network_address.to_string())
            .collect()
    }
}

pub struct ValidatorClusterBuilder {
    genesis_config: Option<GenesisConfig>,
    network_config: Option<NetworkConfig>,
    consensus_rpc_host: Option<String>,
    consensus_rpc_port: Option<u16>,
    additional_objects: Vec<Object>,
    cluster_size: Option<usize>,
    enable_events: bool,
    supported_protocol_versions_config: ProtocolVersionsConfig,
    db_checkpoint_config: DBCheckpointConfig,
    num_unpruned_validators: Option<usize>,
    jwk_fetch_interval: Option<Duration>,
    config_dir: Option<PathBuf>,
    default_jwks: bool,
}

impl ValidatorClusterBuilder {
    pub fn new() -> Self {
        Self {
            genesis_config: None,
            network_config: None,
            consensus_rpc_host: None,
            consensus_rpc_port: None,
            additional_objects: vec![],
            cluster_size: None,
            enable_events: false,
            supported_protocol_versions_config: ProtocolVersionsConfig::Default,
            db_checkpoint_config: DBCheckpointConfig::default(),
            num_unpruned_validators: None,
            jwk_fetch_interval: None,
            config_dir: None,
            default_jwks: false,
        }
    }
    pub fn with_config_dir(mut self, config_dir: PathBuf) -> Self {
        self.config_dir = Some(config_dir);
        self
    }
    pub fn with_epoch_duration_ms(mut self, epoch_duration_ms: u64) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .epoch_duration_ms = epoch_duration_ms;
        self
    }

    pub fn with_cluster_size(mut self, num: usize) -> Self {
        self.cluster_size = Some(num);
        self
    }

    pub fn with_consensus_rpc_host(mut self, consensus_rpc_host: String) -> Self {
        assert!(self.consensus_rpc_host.is_none());
        self.consensus_rpc_host = Some(consensus_rpc_host);
        self
    }
    pub fn with_consensus_rpc_port(mut self, consensus_rpc_port: u16) -> Self {
        assert!(self.consensus_rpc_port.is_none());
        self.consensus_rpc_port = Some(consensus_rpc_port);
        self
    }

    pub fn set_genesis_config(mut self, genesis_config: GenesisConfig) -> Self {
        assert!(self.genesis_config.is_none() && self.network_config.is_none());
        self.genesis_config = Some(genesis_config);
        self
    }

    pub fn set_network_config(mut self, network_config: NetworkConfig) -> Self {
        assert!(self.genesis_config.is_none() && self.network_config.is_none());
        self.network_config = Some(network_config);
        self
    }

    pub async fn build(mut self) -> Result<ValidatorCluster> {
        // All test clusters receive a continuous stream of random JWKs.
        // If we later use zklogin authenticated transactions in tests we will need to supply
        // valid JWKs as well.
        #[cfg(msim)]
        if !self.default_jwks {
            sui_node::set_jwk_injector(Arc::new(|_authority, provider| {
                use fastcrypto_zkp::bn254::zk_login::{JwkId, JWK};
                use rand::Rng;

                // generate random (and possibly conflicting) id/key pairings.
                let id_num = rand::thread_rng().gen_range(1..=4);
                let key_num = rand::thread_rng().gen_range(1..=4);

                let id = JwkId {
                    iss: provider.get_config().iss,
                    kid: format!("kid{}", id_num),
                };

                let jwk = JWK {
                    kty: "kty".to_string(),
                    e: "e".to_string(),
                    n: format!("n{}", key_num),
                    alg: "alg".to_string(),
                };

                Ok(vec![(id, jwk)])
            }));
        }

        let swarm = self.start_swarm().await.unwrap();

        // let validator_node = swarm.validator_node_handles().mapnext().unwrap();
        // let json_rpc_address = validator_node.config.json_rpc_address;
        // let validator_handle =
        //     ValidatorNodeHandle::new(validator_node.get_node_handle().unwrap(), json_rpc_address)
        //         .await;
        Ok(ValidatorCluster {
            swarm,
            config_directory: tempfile::tempdir()?,
        })
    }
    /// Start a Swarm and set up WalletConfig
    async fn start_swarm(&mut self) -> Result<ValidatorSwarm, anyhow::Error> {
        let mut builder: ValidatorSwarmBuilder = ValidatorSwarm::builder()
            .committee_size(NonZeroUsize::new(self.cluster_size.unwrap_or(NUM_VALIDATOR)).unwrap())
            .with_objects(self.additional_objects.clone())
            .with_db_checkpoint_config(self.db_checkpoint_config.clone());

        if let Some(genesis_config) = self.genesis_config.take() {
            builder = builder.with_genesis_config(genesis_config);
        }

        if let Some(network_config) = self.network_config.take() {
            builder = builder.with_network_config(network_config);
        }
        if let Some(consensus_rpc_host) = self.consensus_rpc_host.take() {
            builder = builder.with_consensus_rpc_host(consensus_rpc_host);
        }
        if let Some(consensus_rpc_port) = self.consensus_rpc_port.take() {
            builder = builder.with_consensus_rpc_port(consensus_rpc_port);
        }
        if let Some(num_unpruned_validators) = self.num_unpruned_validators {
            builder = builder.with_num_unpruned_validators(num_unpruned_validators);
        }

        if let Some(jwk_fetch_interval) = self.jwk_fetch_interval {
            builder = builder.with_jwk_fetch_interval(jwk_fetch_interval);
        }

        if let Some(config_dir) = self.config_dir.take() {
            builder = builder.dir(config_dir);
        }

        let mut swarm = builder.build();
        swarm.launch().await?;

        let dir = swarm.dir();

        let network_path = dir.join(SUI_NETWORK_CONFIG);
        let wallet_path = dir.join(SUI_CLIENT_CONFIG);
        let keystore_path = dir.join(SUI_KEYSTORE_FILENAME);

        swarm.config().save(&network_path)?;
        let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path)?);
        for key in &swarm.config().account_keys {
            keystore.add_key(None, SuiKeyPair::Ed25519(key.copy()))?;
        }

        let active_address = keystore.addresses().first().cloned();

        // Create wallet config with stated authorities port
        SuiClientConfig {
            keystore: Keystore::from(FileBasedKeystore::new(&keystore_path)?),
            envs: Default::default(),
            active_address,
            active_env: Default::default(),
        }
        .save(wallet_path)?;

        // Return network handle
        Ok(swarm)
    }
    fn get_or_init_genesis_config(&mut self) -> &mut GenesisConfig {
        if self.genesis_config.is_none() {
            self.genesis_config = Some(GenesisConfig::for_local_testing());
        }
        self.genesis_config.as_mut().unwrap()
    }
}
impl Default for ValidatorClusterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
