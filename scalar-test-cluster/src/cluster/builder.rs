use crate::swarm::{Swarm, SwarmBuilder};
use crate::{FullNodeHandle, TestCluster};
use scalar_swarm_config::genesis_config::{
    AccountConfig, GenesisConfig, ValidatorGenesisConfig, DEFAULT_GAS_AMOUNT,
};
use scalar_swarm_config::network_config::NetworkConfig;
use scalar_swarm_config::network_config_builder::{
    ProtocolVersionsConfig, SupportedProtocolVersionsCallback,
};
use scalar_swarm_config::node_config_builder::{FullnodeConfigBuilder, ValidatorConfigBuilder};
use std::num::NonZeroUsize;
use std::{path::PathBuf, time::Duration};
use sui_config::node::DBCheckpointConfig;
use sui_config::{Config, SUI_CLIENT_CONFIG, SUI_NETWORK_CONFIG};
use sui_config::{NodeConfig, PersistedConfig, SUI_KEYSTORE_FILENAME};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::sui_client_config::{SuiClientConfig, SuiEnv};
use sui_sdk::wallet_context::WalletContext;
use sui_types::crypto::{KeypairTraits, SuiKeyPair};
use sui_types::object::Object;

const NUM_VALIDATOR: usize = 4;
pub struct LocalClusterBuilder {
    genesis_config: Option<GenesisConfig>,
    network_config: Option<NetworkConfig>,
    additional_objects: Vec<Object>,
    num_validators: Option<usize>,
    consensus_grpc_port: Option<u16>,
    fullnode_rpc_port: Option<u16>,
    enable_fullnode_events: bool,
    validator_supported_protocol_versions_config: ProtocolVersionsConfig,
    // Default to validator_supported_protocol_versions_config, but can be overridden.
    fullnode_supported_protocol_versions_config: Option<ProtocolVersionsConfig>,
    db_checkpoint_config_validators: DBCheckpointConfig,
    db_checkpoint_config_fullnodes: DBCheckpointConfig,
    num_unpruned_validators: Option<usize>,
    jwk_fetch_interval: Option<Duration>,
    config_dir: Option<PathBuf>,
    default_jwks: bool,
}
impl LocalClusterBuilder {
    pub fn new() -> Self {
        Self {
            genesis_config: None,
            network_config: None,
            additional_objects: vec![],
            consensus_grpc_port: None,
            fullnode_rpc_port: None,
            num_validators: None,
            enable_fullnode_events: false,
            validator_supported_protocol_versions_config: ProtocolVersionsConfig::Default,
            fullnode_supported_protocol_versions_config: None,
            db_checkpoint_config_validators: DBCheckpointConfig::default(),
            db_checkpoint_config_fullnodes: DBCheckpointConfig::default(),
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

    pub fn with_num_validators(mut self, num: usize) -> Self {
        self.num_validators = Some(num);
        self
    }

    pub fn with_consensus_grpc_port(mut self, port: Option<u16>) -> Self {
        self.consensus_grpc_port = port;
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

    pub async fn build(mut self) -> TestCluster {
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
        let working_dir = swarm.dir();

        // let mut wallet_conf: SuiClientConfig =
        //     PersistedConfig::read(&working_dir.join(SUI_CLIENT_CONFIG)).unwrap();

        // let fullnode = swarm.fullnodes().next().unwrap();
        // let json_rpc_address = fullnode.config.json_rpc_address;
        // let fullnode_handle =
        //     FullNodeHandle::new(fullnode.get_node_handle().unwrap(), json_rpc_address).await;

        // wallet_conf.envs.push(SuiEnv {
        //     alias: "localnet".to_string(),
        //     rpc: fullnode_handle.rpc_url.clone(),
        //     ws: Some(fullnode_handle.ws_url.clone()),
        // });
        // wallet_conf.active_env = Some("localnet".to_string());

        // wallet_conf
        //     .persisted(&working_dir.join(SUI_CLIENT_CONFIG))
        //     .save()
        //     .unwrap();

        // let wallet_conf = swarm.dir().join(SUI_CLIENT_CONFIG);
        // let wallet = WalletContext::new(&wallet_conf, None, None).await.unwrap();

        TestCluster {
            swarm,
            // wallet,
            // fullnode_handle,
        }
    }
    /// Start a Swarm and set up WalletConfig
    async fn start_swarm(&mut self) -> Result<Swarm, anyhow::Error> {
        let mut builder: SwarmBuilder = Swarm::builder()
            .committee_size(
                NonZeroUsize::new(self.num_validators.unwrap_or(NUM_VALIDATOR)).unwrap(),
            )
            .with_objects(self.additional_objects.clone())
            .with_db_checkpoint_config(self.db_checkpoint_config_validators.clone())
            .with_supported_protocol_versions_config(
                self.validator_supported_protocol_versions_config.clone(),
            )
            .with_fullnode_count(1)
            .with_fullnode_supported_protocol_versions_config(
                self.fullnode_supported_protocol_versions_config
                    .clone()
                    .unwrap_or(self.validator_supported_protocol_versions_config.clone()),
            )
            .with_db_checkpoint_config(self.db_checkpoint_config_fullnodes.clone());

        if let Some(genesis_config) = self.genesis_config.take() {
            builder = builder.with_genesis_config(genesis_config);
        }

        if let Some(network_config) = self.network_config.take() {
            builder = builder.with_network_config(network_config);
        }
        if let Some(consensus_grpc_port) = self.consensus_grpc_port {
            builder = builder.with_consensus_rpc_port(consensus_grpc_port);
        }
        if let Some(fullnode_rpc_port) = self.fullnode_rpc_port {
            builder = builder.with_fullnode_rpc_port(fullnode_rpc_port);
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

impl Default for LocalClusterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
