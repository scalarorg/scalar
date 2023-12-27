// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::SwarmDirectory;

use super::ValidatorNode;
use anyhow::Result;
use futures::future::try_join_all;
use rand::rngs::OsRng;
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::time::Duration;
use std::{
    mem, ops,
    path::{Path, PathBuf},
};
use sui_config::node::{DBCheckpointConfig, OverloadThresholdConfig};
use sui_config::NodeConfig;
use sui_node::SuiNodeHandle;
use sui_protocol_config::{ProtocolVersion, SupportedProtocolVersions};
use sui_swarm_config::genesis_config::{AccountConfig, GenesisConfig, ValidatorGenesisConfig};
use sui_swarm_config::network_config::NetworkConfig;
use sui_swarm_config::network_config_builder::{
    CommitteeConfig, ConfigBuilder, ProtocolVersionsConfig, SupportedProtocolVersionsCallback,
};
use sui_swarm_config::node_config_builder::FullnodeConfigBuilder;
use sui_types::base_types::AuthorityName;
use sui_types::object::Object;
use tempfile::TempDir;
use tracing::info;
pub struct ValidatorSwarmBuilder<R = OsRng> {
    rng: R,
    // template: NodeConfig,
    dir: Option<PathBuf>,
    committee: CommitteeConfig,
    consensus_rpc_host: Option<String>,
    consensus_rpc_port: Option<u16>,
    genesis_config: Option<GenesisConfig>,
    network_config: Option<NetworkConfig>,
    additional_objects: Vec<Object>,
    supported_protocol_versions_config: ProtocolVersionsConfig,
    db_checkpoint_config: DBCheckpointConfig,
    jwk_fetch_interval: Option<Duration>,
    num_unpruned_validators: Option<usize>,
    overload_threshold_config: Option<OverloadThresholdConfig>,
    data_ingestion_dir: Option<PathBuf>,
}

impl ValidatorSwarmBuilder {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            rng: OsRng,
            dir: None,
            committee: CommitteeConfig::Size(NonZeroUsize::new(1).unwrap()),
            consensus_rpc_host: None,
            consensus_rpc_port: None,
            genesis_config: None,
            network_config: None,
            additional_objects: vec![],
            supported_protocol_versions_config: ProtocolVersionsConfig::Default,
            db_checkpoint_config: DBCheckpointConfig::default(),
            jwk_fetch_interval: None,
            num_unpruned_validators: None,
            overload_threshold_config: None,
            data_ingestion_dir: None,
        }
    }
}

impl<R> ValidatorSwarmBuilder<R> {
    pub fn rng<N: rand::RngCore + rand::CryptoRng>(self, rng: N) -> ValidatorSwarmBuilder<N> {
        ValidatorSwarmBuilder {
            rng,
            dir: self.dir,
            committee: self.committee,
            consensus_rpc_host: self.consensus_rpc_host,
            consensus_rpc_port: self.consensus_rpc_port,
            genesis_config: self.genesis_config,
            network_config: self.network_config,
            additional_objects: self.additional_objects,
            supported_protocol_versions_config: self.supported_protocol_versions_config,
            db_checkpoint_config: self.db_checkpoint_config,
            jwk_fetch_interval: self.jwk_fetch_interval,
            num_unpruned_validators: self.num_unpruned_validators,
            overload_threshold_config: self.overload_threshold_config,
            data_ingestion_dir: self.data_ingestion_dir,
        }
    }

    /// Set the directory that should be used by the Swarm for any on-disk data.
    ///
    /// If a directory is provided, it will not be cleaned up when the Swarm is dropped.
    ///
    /// Defaults to using a temporary directory that will be cleaned up when the Swarm is dropped.
    pub fn dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.dir = Some(dir.into());
        self
    }

    /// Set the committee size (the number of validators in the validator set).
    ///
    /// Defaults to 1.
    pub fn committee_size(mut self, committee_size: NonZeroUsize) -> Self {
        self.committee = CommitteeConfig::Size(committee_size);
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
    pub fn with_validators(mut self, validators: Vec<ValidatorGenesisConfig>) -> Self {
        self.committee = CommitteeConfig::Validators(validators);
        self
    }

    pub fn with_genesis_config(mut self, genesis_config: GenesisConfig) -> Self {
        assert!(self.network_config.is_none() && self.genesis_config.is_none());
        self.genesis_config = Some(genesis_config);
        self
    }

    pub fn with_num_unpruned_validators(mut self, n: usize) -> Self {
        assert!(self.network_config.is_none());
        self.num_unpruned_validators = Some(n);
        self
    }

    pub fn with_jwk_fetch_interval(mut self, i: Duration) -> Self {
        self.jwk_fetch_interval = Some(i);
        self
    }

    pub fn with_network_config(mut self, network_config: NetworkConfig) -> Self {
        assert!(self.network_config.is_none() && self.genesis_config.is_none());
        self.network_config = Some(network_config);
        self
    }

    pub fn with_accounts(mut self, accounts: Vec<AccountConfig>) -> Self {
        self.get_or_init_genesis_config().accounts = accounts;
        self
    }

    pub fn with_objects<I: IntoIterator<Item = Object>>(mut self, objects: I) -> Self {
        self.additional_objects.extend(objects);
        self
    }

    pub fn with_epoch_duration_ms(mut self, epoch_duration_ms: u64) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .epoch_duration_ms = epoch_duration_ms;
        self
    }

    pub fn with_protocol_version(mut self, v: ProtocolVersion) -> Self {
        self.get_or_init_genesis_config()
            .parameters
            .protocol_version = v;
        self
    }

    pub fn with_supported_protocol_versions(mut self, c: SupportedProtocolVersions) -> Self {
        self.supported_protocol_versions_config = ProtocolVersionsConfig::Global(c);
        self
    }

    pub fn with_supported_protocol_version_callback(
        mut self,
        func: SupportedProtocolVersionsCallback,
    ) -> Self {
        self.supported_protocol_versions_config = ProtocolVersionsConfig::PerValidator(func);
        self
    }

    pub fn with_supported_protocol_versions_config(mut self, c: ProtocolVersionsConfig) -> Self {
        self.supported_protocol_versions_config = c;
        self
    }

    pub fn with_db_checkpoint_config(mut self, db_checkpoint_config: DBCheckpointConfig) -> Self {
        self.db_checkpoint_config = db_checkpoint_config;
        self
    }

    pub fn with_overload_threshold_config(
        mut self,
        overload_threshold_config: OverloadThresholdConfig,
    ) -> Self {
        assert!(self.network_config.is_none());
        self.overload_threshold_config = Some(overload_threshold_config);
        self
    }

    pub fn with_data_ingestion_dir(mut self, path: PathBuf) -> Self {
        self.data_ingestion_dir = Some(path);
        self
    }

    fn get_or_init_genesis_config(&mut self) -> &mut GenesisConfig {
        if self.genesis_config.is_none() {
            assert!(self.network_config.is_none());
            self.genesis_config = Some(GenesisConfig::for_local_testing());
        }
        self.genesis_config.as_mut().unwrap()
    }
}

impl<R: rand::RngCore + rand::CryptoRng> ValidatorSwarmBuilder<R> {
    /// Create the configured Swarm.
    pub fn build(self) -> ValidatorSwarm {
        let dir = if let Some(dir) = self.dir {
            SwarmDirectory::Persistent(dir)
        } else {
            SwarmDirectory::Temporary(TempDir::new().unwrap())
        };
        let grpc_host = self.consensus_rpc_host.as_ref();
        let grpc_port = self.consensus_rpc_port.as_ref();
        let network_config = self.network_config.unwrap_or_else(|| {
            let mut config_builder = ConfigBuilder::new(dir.as_ref());

            if let Some(genesis_config) = self.genesis_config {
                config_builder = config_builder.with_genesis_config(genesis_config);
            }

            if let Some(num_unpruned_validators) = self.num_unpruned_validators {
                config_builder =
                    config_builder.with_num_unpruned_validators(num_unpruned_validators);
            }

            if let Some(jwk_fetch_interval) = self.jwk_fetch_interval {
                config_builder = config_builder.with_jwk_fetch_interval(jwk_fetch_interval);
            }

            if let Some(overload_threshold_config) = self.overload_threshold_config {
                config_builder =
                    config_builder.with_overload_threshold_config(overload_threshold_config);
            }

            if let Some(path) = self.data_ingestion_dir {
                config_builder = config_builder.with_data_ingestion_dir(path);
            }

            config_builder
                .committee(self.committee)
                .rng(self.rng)
                .with_objects(self.additional_objects)
                .with_supported_protocol_versions_config(
                    self.supported_protocol_versions_config.clone(),
                )
                .build()
        });
        let mut nodes: HashMap<_, _> = network_config
            .validator_configs()
            .iter()
            .enumerate()
            .map(|(ind, config)| {
                let mut config = config.to_owned();
                if let (Some(host), Some(port)) = (grpc_host, grpc_port) {
                    let network_address = format!(
                        "/ip4/{}/tcp/{}/http",
                        //Ipv4Addr::UNSPECIFIED,
                        host,
                        port.clone() + (ind as u16)
                    );
                    info!("Network address {network_address}");
                    config.network_address = network_address.parse().unwrap();
                } else if let Some(port) = grpc_port {
                    let network_address = format!(
                        "/ip4/{}/tcp/{}/http",
                        Ipv4Addr::LOCALHOST,
                        port.clone() + (ind as u16)
                    );
                    info!("Network address {network_address}");
                    config.network_address = network_address.parse().unwrap();
                }
                (config.protocol_public_key(), ValidatorNode::new(config))
            })
            .collect();
        ValidatorSwarm {
            dir,
            network_config,
            nodes,
        }
    }
}

/// A handle to an in-memory Sui Network.
#[derive(Debug)]
pub struct ValidatorSwarm {
    dir: SwarmDirectory,
    network_config: NetworkConfig,
    nodes: HashMap<AuthorityName, ValidatorNode>,
}

impl Drop for ValidatorSwarm {
    fn drop(&mut self) {
        self.nodes_iter_mut().for_each(|node| node.stop());
    }
}

impl ValidatorSwarm {
    fn nodes_iter_mut(&mut self) -> impl Iterator<Item = &mut ValidatorNode> {
        self.nodes.values_mut()
    }

    /// Return a new Builder
    pub fn builder() -> ValidatorSwarmBuilder {
        ValidatorSwarmBuilder::new()
    }

    /// Start all nodes associated with this Swarm
    pub async fn launch(&mut self) -> Result<()> {
        try_join_all(self.nodes_iter_mut().map(|node| node.start())).await?;
        tracing::info!("Successfully launched Swarm");
        Ok(())
    }

    /// Return the path to the directory where this Swarm's on-disk data is kept.
    pub fn dir(&self) -> &Path {
        self.dir.as_ref()
    }

    /// Ensure that the Swarm data directory will persist and not be cleaned up when this Swarm is
    /// dropped.
    pub fn persist_dir(&mut self) {
        self.dir.persist();
    }

    /// Return a reference to this Swarm's `NetworkConfig`.
    pub fn config(&self) -> &NetworkConfig {
        &self.network_config
    }

    /// Return a mutable reference to this Swarm's `NetworkConfig`.
    // TODO: It's not ideal to mutate network config. We should consider removing this.
    pub fn config_mut(&mut self) -> &mut NetworkConfig {
        &mut self.network_config
    }

    pub fn all_nodes(&self) -> impl Iterator<Item = &ValidatorNode> {
        self.nodes.values()
    }

    pub fn node(&self, name: &AuthorityName) -> Option<&ValidatorNode> {
        self.nodes.get(name)
    }

    pub fn node_mut(&mut self, name: &AuthorityName) -> Option<&mut ValidatorNode> {
        self.nodes.get_mut(name)
    }

    /// Return an iterator over shared references of all nodes that are set up as validators.
    /// This means that they have a consensus config. This however doesn't mean this validator is
    /// currently active (i.e. it's not necessarily in the validator set at the moment).
    pub fn validator_nodes(&self) -> impl Iterator<Item = &ValidatorNode> {
        self.nodes
            .values()
            .filter(|node| node.config.consensus_config.is_some())
    }

    pub fn validator_node_handles(&self) -> Vec<SuiNodeHandle> {
        self.validator_nodes()
            .map(|node| node.get_node_handle().unwrap())
            .collect()
    }

    /// Returns an iterator over all currently active validators.
    pub fn active_validators(&self) -> impl Iterator<Item = &ValidatorNode> {
        self.validator_nodes().filter(|node| {
            node.get_node_handle().map_or(false, |handle| {
                let state = handle.state();
                state.is_validator(&state.epoch_store_for_testing())
            })
        })
    }

    pub async fn spawn_new_node(&mut self, config: NodeConfig) -> SuiNodeHandle {
        let name = config.protocol_public_key();
        let node = ValidatorNode::new(config);
        node.start().await.unwrap();
        let handle = node.get_node_handle().unwrap();
        self.nodes.insert(name, node);
        handle
    }
}

#[cfg(test)]
mod test {
    use super::ValidatorSwarm;
    use std::num::NonZeroUsize;

    #[tokio::test]
    async fn launch() {
        telemetry_subscribers::init_for_testing();
        let mut swarm = ValidatorSwarm::builder()
            .committee_size(NonZeroUsize::new(4).unwrap())
            .build();

        swarm.launch().await.unwrap();

        for validator in swarm.validator_nodes() {
            validator.health_check(true).await.unwrap();
        }
    }
}
