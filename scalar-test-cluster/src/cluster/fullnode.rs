use super::{ClusterTrait, FullnodeClusterTrait};
use crate::LocalClusterConfig;
use crate::NUM_VALIDATOR;
use anyhow::Result;
use async_trait::async_trait;
use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    ws_client::{WsClient, WsClientBuilder},
};
use scalar_node::SuiNodeHandle;
use scalar_swarm::fullnode::{FullnodeSwarm, FullnodeSwarmBuilder};
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
use sui_graphql_rpc::config::ConnectionConfig;
use sui_graphql_rpc::test_infra::cluster::start_graphql_server;
use sui_indexer::test_utils::{start_test_indexer, start_test_indexer_v2};
use sui_indexer::IndexerConfig;
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::sui_client_config::SuiEnv;
use sui_sdk::{
    sui_client_config::SuiClientConfig, wallet_context::WalletContext, SuiClient, SuiClientBuilder,
};
use sui_types::{
    base_types::SuiAddress,
    crypto::{get_key_pair, AccountKeyPair, KeypairTraits, SuiKeyPair},
    object::Object,
};
use tracing::{error, info};

pub struct FullNodeHandle {
    pub sui_node: SuiNodeHandle,
    pub sui_client: SuiClient,
    pub rpc_client: HttpClient,
    pub rpc_url: String,
    pub ws_url: String,
}

impl FullNodeHandle {
    pub async fn new(sui_node: SuiNodeHandle, json_rpc_address: SocketAddr) -> Self {
        let rpc_url = format!("http://{}", json_rpc_address);
        let rpc_client = HttpClientBuilder::default().build(&rpc_url).unwrap();

        let ws_url = format!("ws://{}", json_rpc_address);
        let sui_client = SuiClientBuilder::default().build(&rpc_url).await.unwrap();

        Self {
            sui_node,
            sui_client,
            rpc_client,
            rpc_url,
            ws_url,
        }
    }

    pub async fn ws_client(&self) -> WsClient {
        WsClientBuilder::default()
            .build(&self.ws_url)
            .await
            .unwrap()
    }
}

pub struct FullnodeCluster {
    pub swarm: FullnodeSwarm,
    pub wallet: WalletContext,
    pub fullnode_handle: FullNodeHandle,
    indexer_url: Option<String>,
    faucet_key: AccountKeyPair,
    config_directory: tempfile::TempDir,
}

impl FullnodeCluster {
    #[allow(unused)]
    pub fn swarm(&self) -> &FullnodeSwarm {
        &self.swarm
    }
}

#[async_trait]
impl ClusterTrait for FullnodeCluster {
    async fn start(options: &LocalClusterConfig) -> Result<Self> {
        // TODO: options should contain port instead of address
        let fullnode_port = options.fullnode_address.as_ref().map(|addr| {
            addr.parse::<SocketAddr>()
                .expect("Unable to parse fullnode address")
                .port()
        });

        let indexer_address = options.indexer_address.as_ref().map(|addr| {
            addr.parse::<SocketAddr>()
                .expect("Unable to parse indexer address")
        });

        let mut cluster_builder = FullnodeClusterBuilder::new(); //.enable_fullnode_events();
        if let Some(size) = options.cluster_size.as_ref() {
            cluster_builder = cluster_builder.with_cluster_size(size.clone());
        }
        if let (Some(host), Some(port)) = (
            options.consensus_rpc_host.as_ref(),
            options.consensus_rpc_port.as_ref(),
        ) {
            let consensus_url = format!("http://{}:{}", host, port);
            cluster_builder = cluster_builder.with_consensus_url(consensus_url);
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

        if let Some(rpc_port) = fullnode_port {
            cluster_builder = cluster_builder.with_fullnode_rpc_port(rpc_port);
        }

        let mut fullnode_cluster = cluster_builder.build().await?;
        fullnode_cluster.indexer_url = options.indexer_address.clone();
        // Use the wealthy account for faucet
        let faucet_key = fullnode_cluster
            .swarm
            .config_mut()
            .account_keys
            .swap_remove(0);
        let faucet_address = SuiAddress::from(faucet_key.public());
        info!(?faucet_address, "faucet_address");

        // This cluster has fullnode handle, safe to unwrap
        let fullnode_url = fullnode_cluster.fullnode_handle.rpc_url.clone();

        if let (Some(pg_address), Some(indexer_address)) =
            (options.pg_address.clone(), indexer_address)
        {
            if options.use_indexer_v2 {
                // Start in writer mode
                start_test_indexer_v2(
                    Some(pg_address.clone()),
                    fullnode_url.clone(),
                    None,
                    options.use_indexer_experimental_methods,
                )
                .await;

                // Start in reader mode
                start_test_indexer_v2(
                    Some(pg_address),
                    fullnode_url.clone(),
                    Some(indexer_address.to_string()),
                    options.use_indexer_experimental_methods,
                )
                .await;
            } else {
                let migrated_methods = if options.use_indexer_experimental_methods {
                    IndexerConfig::all_implemented_methods()
                } else {
                    vec![]
                };
                let config = IndexerConfig {
                    db_url: Some(pg_address),
                    rpc_client_url: fullnode_url.clone(),
                    rpc_server_url: indexer_address.ip().to_string(),
                    rpc_server_port: indexer_address.port(),
                    migrated_methods,
                    reset_db: true,
                    ..Default::default()
                };
                start_test_indexer(config).await?;
            }
        }

        if let Some(graphql_address) = &options.graphql_address {
            let graphql_address = graphql_address.parse::<SocketAddr>()?;
            let graphql_connection_config = ConnectionConfig::new(
                Some(graphql_address.port()),
                Some(graphql_address.ip().to_string()),
                options.pg_address.clone(),
                None,
                None,
                None,
            );

            start_graphql_server(graphql_connection_config.clone()).await;
        }

        // Let nodes connect to one another
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // TODO: test connectivity before proceeding?
        Ok(fullnode_cluster)
    }

    fn user_key(&self) -> AccountKeyPair {
        get_key_pair().1
    }

    fn config_directory(&self) -> &Path {
        self.config_directory.path()
    }
}
#[async_trait]
impl FullnodeClusterTrait for FullnodeCluster {
    fn fullnode_url(&self) -> &str {
        self.fullnode_handle.rpc_url.as_str()
    }

    fn indexer_url(&self) -> &Option<String> {
        &self.indexer_url
    }

    fn remote_faucet_url(&self) -> Option<&str> {
        None
    }

    fn local_faucet_key(&self) -> Option<&AccountKeyPair> {
        Some(&self.faucet_key)
    }
}
pub struct FullnodeClusterBuilder {
    genesis_config: Option<GenesisConfig>,
    network_config: Option<NetworkConfig>,
    additional_objects: Vec<Object>,
    cluster_size: Option<usize>,
    ///Rpc port for external rpc api call
    fullnode_rpc_port: Option<u16>,
    enable_events: bool,
    consensus_url: Option<String>,
    supported_protocol_versions_config: ProtocolVersionsConfig,
    db_checkpoint_config: DBCheckpointConfig,
    num_unpruned_validators: Option<usize>,
    jwk_fetch_interval: Option<Duration>,
    config_dir: Option<PathBuf>,
    default_jwks: bool,
}

impl FullnodeClusterBuilder {
    pub fn new() -> Self {
        Self {
            genesis_config: None,
            network_config: None,
            additional_objects: vec![],
            cluster_size: None,
            fullnode_rpc_port: None,
            consensus_url: None,
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

    pub fn with_fullnode_rpc_port(mut self, port: u16) -> Self {
        self.fullnode_rpc_port = Some(port);
        self
    }

    pub fn with_consensus_url(mut self, url: String) -> Self {
        self.consensus_url = Some(url);
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

    pub async fn build(mut self) -> Result<FullnodeCluster> {
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

        let mut swarm = self.start_swarm().await.unwrap();
        let working_dir = swarm.dir();

        let mut wallet_conf: SuiClientConfig =
            PersistedConfig::read(&working_dir.join(SUI_CLIENT_CONFIG)).unwrap();

        let fullnode = swarm.fullnodes().next().unwrap();
        let json_rpc_address = fullnode.config.json_rpc_address;
        let fullnode_handle =
            FullNodeHandle::new(fullnode.get_node_handle().unwrap(), json_rpc_address).await;
        wallet_conf.envs.push(SuiEnv {
            alias: "localnet".to_string(),
            rpc: fullnode_handle.rpc_url.clone(),
            ws: Some(fullnode_handle.ws_url.clone()),
        });
        wallet_conf.active_env = Some("localnet".to_string());

        wallet_conf
            .persisted(&working_dir.join(SUI_CLIENT_CONFIG))
            .save()
            .unwrap();

        let wallet_conf = working_dir.join(SUI_CLIENT_CONFIG);
        let wallet = WalletContext::new(&wallet_conf, None, None).await.unwrap();
        let faucet_key = swarm.config_mut().account_keys.swap_remove(0);
        Ok(FullnodeCluster {
            swarm,
            fullnode_handle,
            config_directory: tempfile::tempdir()?,
            wallet,
            indexer_url: None,
            faucet_key,
        })
    }
    /// Start a Swarm and set up WalletConfig
    async fn start_swarm(&mut self) -> Result<FullnodeSwarm, anyhow::Error> {
        let mut builder: FullnodeSwarmBuilder = FullnodeSwarm::builder()
            .committee_size(NonZeroUsize::new(self.cluster_size.unwrap_or(NUM_VALIDATOR)).unwrap())
            .with_objects(self.additional_objects.clone())
            .with_fullnode_count(1)
            .with_fullnode_supported_protocol_versions_config(
                self.supported_protocol_versions_config.clone(),
            )
            .with_db_checkpoint_config(self.db_checkpoint_config.clone());

        if let Some(genesis_config) = self.genesis_config.take() {
            builder = builder.with_genesis_config(genesis_config);
        }

        if let Some(network_config) = self.network_config.take() {
            builder = builder.with_network_config(network_config);
        }
        if let Some(consensus_url) = self.consensus_url.take() {
            builder = builder.with_consensus_url(consensus_url);
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
impl Default for FullnodeClusterBuilder {
    fn default() -> Self {
        Self::new()
    }
}
