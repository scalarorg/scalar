use super::validator_component::ValidatorComponents;
use crate::metrics::SuiNodeMetrics;
use anemo::Network;
use anemo_tower::callback::CallbackLayer;
use anemo_tower::trace::DefaultMakeSpan;
use anemo_tower::trace::DefaultOnFailure;
use anemo_tower::trace::TraceLayer;
use anyhow::anyhow;
use anyhow::Result;
use arc_swap::ArcSwap;
use mysten_metrics::RegistryService;
use narwhal_network::metrics::{
    MetricsMakeCallbackHandler, NetworkConnectionMetrics, NetworkMetrics,
};
use prometheus::Registry;
use scalar_core::authority::authority_per_epoch_store::AuthorityPerEpochStore;
use scalar_core::authority::authority_store_tables::AuthorityPerpetualTables;
use scalar_core::authority::AuthorityState;
use scalar_core::authority::AuthorityStore;
use scalar_core::authority::CHAIN_IDENTIFIER;
use scalar_core::checkpoints::CheckpointStore;
use scalar_core::consensus_adapter::ConnectionMonitorStatus;
use scalar_core::db_checkpoint_handler::DBCheckpointHandler;
use scalar_core::epoch::committee_store::CommitteeStore;
use scalar_core::epoch::epoch_metrics::EpochMetrics;
use scalar_core::epoch::reconfiguration::ReconfigurationInitiator;
use scalar_core::module_cache_metrics::ResolverMetrics;
use scalar_core::signature_verifier::SignatureVerifierMetrics;
use scalar_core::{state_accumulator::StateAccumulator, storage::RocksDbStore};
use std::collections::HashMap;
use std::{fmt, sync::Arc};
use sui_archival::reader::ArchiveReaderBalancer;
use sui_config::node::DBCheckpointConfig;
use sui_config::NodeConfig;
use sui_network::discovery;
use sui_network::discovery::TrustedPeerChangeEvent;
use sui_network::state_sync;
use sui_protocol_config::SupportedProtocolVersions;
use sui_snapshot::uploader::StateSnapshotUploader;
use sui_types::base_types::ConciseableName;
use sui_types::committee::EpochId;
use sui_types::crypto::KeypairTraits;
use sui_types::digests::ChainIdentifier;
use sui_types::error::SuiResult;
use sui_types::sui_system_state::epoch_start_sui_system_state::EpochStartSystemStateTrait;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tracing::info;
use tracing::warn;
use typed_store::rocks::default_db_options;
use typed_store::DBMetrics;

pub struct ScalarNode {
    pub config: NodeConfig,
    validator_node: Mutex<ValidatorComponents>,
    state: Arc<AuthorityState>,
}

impl fmt::Debug for ScalarNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ScalarNode")
            .field("name", &self.state.name.concise())
            .finish()
    }
}

impl ScalarNode {
    pub async fn start(
        config: &NodeConfig,
        registry_service: RegistryService,
    ) -> Result<Arc<ScalarNode>> {
        if config.consensus_config.is_some() {
            Self::start_async(config, registry_service).await
        } else {
            info!("Start scalar node without consensus config");
            Err(anyhow!("Missing consensus config"))
        }
    }
    pub async fn start_async(
        config: &NodeConfig,
        registry_service: RegistryService,
    ) -> Result<Arc<ScalarNode>> {
        let mut config = config.clone();
        if config.supported_protocol_versions.is_none() {
            info!(
                "populating config.supported_protocol_versions with default {:?}",
                SupportedProtocolVersions::SYSTEM_DEFAULT
            );
            config.supported_protocol_versions = Some(SupportedProtocolVersions::SYSTEM_DEFAULT);
        }
        let prometheus_registry = registry_service.default_registry();

        info!(node =? config.protocol_public_key(),
            "Initializing sui-node listening on {}", config.network_address
        );

        // Initialize metrics to track db usage before creating any stores
        DBMetrics::init(&prometheus_registry);
        mysten_metrics::init_metrics(&prometheus_registry);

        let genesis = config.genesis()?;

        let secret = Arc::pin(config.protocol_key_pair().copy());
        let genesis_committee = genesis.committee()?;
        let committee_store = Arc::new(CommitteeStore::new(
            config.db_path().join("epochs"),
            &genesis_committee,
            None,
        ));

        let perpetual_options = default_db_options().optimize_db_for_write_throughput(4);
        let perpetual_tables = Arc::new(AuthorityPerpetualTables::open(
            &config.db_path().join("store"),
            Some(perpetual_options.options),
        ));
        let is_genesis = perpetual_tables
            .database_is_empty()
            .expect("Database read should not fail at init.");
        let store = AuthorityStore::open(
            perpetual_tables,
            genesis,
            &committee_store,
            config.indirect_objects_threshold,
            config
                .expensive_safety_check_config
                .enable_epoch_sui_conservation_check(),
            &prometheus_registry,
        )
        .await?;
        let cur_epoch = store.get_recovery_epoch_at_restart()?;
        let committee = committee_store
            .get_committee(&cur_epoch)?
            .expect("Committee of the current epoch must exist");
        let epoch_start_configuration = store
            .get_epoch_start_configuration()?
            .expect("EpochStartConfiguration of the current epoch must exist");
        let cache_metrics = Arc::new(ResolverMetrics::new(&prometheus_registry));
        let signature_verifier_metrics = SignatureVerifierMetrics::new(&prometheus_registry);

        let epoch_options = default_db_options().optimize_db_for_write_throughput(4);
        let epoch_store = AuthorityPerEpochStore::new(
            config.protocol_public_key(),
            committee.clone(),
            &config.db_path().join("store"),
            Some(epoch_options.options),
            EpochMetrics::new(&registry_service.default_registry()),
            epoch_start_configuration,
            store.clone(),
            cache_metrics,
            signature_verifier_metrics,
            &config.expensive_safety_check_config,
            ChainIdentifier::from(*genesis.checkpoint().digest()),
        );
        // the database is empty at genesis time
        if is_genesis {
            // When we are opening the db table, the only time when it's safe to
            // check SUI conservation is at genesis. Otherwise we may be in the middle of
            // an epoch and the SUI conservation check will fail. This also initialize
            // the expected_network_sui_amount table.
            store
                .expensive_check_sui_conservation(&epoch_store)
                .expect("SUI conservation check cannot fail at genesis");
        }

        let effective_buffer_stake = epoch_store.get_effective_buffer_stake_bps();
        let default_buffer_stake = epoch_store
            .protocol_config()
            .buffer_stake_for_protocol_upgrade_bps();
        if effective_buffer_stake != default_buffer_stake {
            warn!(
                ?effective_buffer_stake,
                ?default_buffer_stake,
                "buffer_stake_for_protocol_upgrade_bps is currently overridden"
            );
        }

        let checkpoint_store = CheckpointStore::new(&config.db_path().join("checkpoints"));
        checkpoint_store.insert_genesis_checkpoint(
            genesis.checkpoint(),
            genesis.checkpoint_contents().clone(),
            &epoch_store,
        );

        let state_sync_store = RocksDbStore::new(
            store.clone(),
            committee_store.clone(),
            checkpoint_store.clone(),
        );

        //Index store if optional use in fullnode configuration
        let index_store = None;

        let chain_identifier = ChainIdentifier::from(*genesis.checkpoint().digest());
        // It's ok if the value is already set due to data races.
        let _ = CHAIN_IDENTIFIER.set(chain_identifier);

        // Create network
        // TODO only configure validators as seed/preferred peers for validators and not for
        // fullnodes once we've had a chance to re-work fullnode configuration generation.
        let archive_readers =
            ArchiveReaderBalancer::new(config.archive_reader_config(), &prometheus_registry)?;
        let (trusted_peer_change_tx, trusted_peer_change_rx) = watch::channel(Default::default());
        let (p2p_network, discovery_handle, state_sync_handle) = Self::create_p2p_network(
            &config,
            state_sync_store.clone(),
            chain_identifier,
            trusted_peer_change_rx,
            archive_readers.clone(),
            &prometheus_registry,
        )?;

        // Start uploading state snapshot to remote store
        let state_snapshot_handle = Self::start_state_snapshot(&config, &prometheus_registry)?;

        // Start uploading db checkpoints to remote store
        let (db_checkpoint_config, db_checkpoint_handle) = Self::start_db_checkpoint(
            &config,
            &prometheus_registry,
            state_snapshot_handle.is_some(),
        )?;

        let mut pruning_config = config.authority_store_pruning_config;
        if !epoch_store
            .protocol_config()
            .simplified_unwrap_then_delete()
        {
            // We cannot prune tombstones if simplified_unwrap_then_delete is not enabled.
            pruning_config.set_killswitch_tombstone_pruning(true);
        }
        /*
         * 231206 - Taivv
         * Modify struct AuthorityState, để loại bỏ các logic của SUI. Chỉ giữ lại các phần liên quan tới Consensus
         */
        let state = AuthorityState::new(
            config.protocol_public_key(),
            secret,
            config.supported_protocol_versions.unwrap(),
            store.clone(),
            epoch_store.clone(),
            committee_store.clone(),
            index_store.clone(),
            checkpoint_store.clone(),
            &prometheus_registry,
            pruning_config,
            genesis.objects(),
            &db_checkpoint_config,
            config.expensive_safety_check_config.clone(),
            config.transaction_deny_config.clone(),
            config.certificate_deny_config.clone(),
            config.indirect_objects_threshold,
            config.state_debug_dump_config.clone(),
            config.overload_threshold_config.clone(),
            archive_readers,
        )
        .await;

        let accumulator = Arc::new(StateAccumulator::new(store));
        let authority_names_to_peer_ids = epoch_store
            .epoch_start_state()
            .get_authority_names_to_peer_ids();

        let network_connection_metrics =
            NetworkConnectionMetrics::new("scalar", &registry_service.default_registry());

        let authority_names_to_peer_ids = ArcSwap::from_pointee(authority_names_to_peer_ids);
        let (_connection_monitor_handle, connection_statuses) =
            narwhal_network::connectivity::ConnectionMonitor::spawn(
                p2p_network.downgrade(),
                network_connection_metrics,
                HashMap::new(),
                None,
            );

        let connection_monitor_status = ConnectionMonitorStatus {
            connection_statuses,
            authority_names_to_peer_ids,
        };
        let connection_monitor_status = Arc::new(connection_monitor_status);
        let sui_node_metrics = Arc::new(SuiNodeMetrics::new(&registry_service.default_registry()));
        match ValidatorComponents::start(
            &config,
            state.clone(),
            committee,
            epoch_store.clone(),
            checkpoint_store.clone(),
            state_sync_handle.clone(),
            accumulator.clone(),
            connection_monitor_status.clone(),
            &registry_service,
            sui_node_metrics.clone(),
        )
        .await
        {
            Ok(validator_node) => {
                let node = Self {
                    config,
                    validator_node: Mutex::new(validator_node),
                    state,
                };
                info!("ScalarNode started!");
                let node = Arc::new(node);
                let node_copy = node.clone();
                // spawn_monitored_task!(async move { Self::monitor_reconfiguration(node_copy).await });

                Ok(node)
            }
            Err(err) => Err(anyhow!("{:?}", err)),
        }
    }

    fn start_state_snapshot(
        config: &NodeConfig,
        prometheus_registry: &Registry,
    ) -> Result<Option<tokio::sync::broadcast::Sender<()>>> {
        if let Some(remote_store_config) = &config.state_snapshot_write_config.object_store_config {
            let snapshot_uploader = StateSnapshotUploader::new(
                &config.db_checkpoint_path(),
                &config.snapshot_path(),
                remote_store_config.clone(),
                60,
                prometheus_registry,
            )?;
            Ok(Some(snapshot_uploader.start()))
        } else {
            Ok(None)
        }
    }

    fn start_db_checkpoint(
        config: &NodeConfig,
        prometheus_registry: &Registry,
        state_snapshot_enabled: bool,
    ) -> Result<(
        DBCheckpointConfig,
        Option<tokio::sync::broadcast::Sender<()>>,
    )> {
        let checkpoint_path = Some(
            config
                .db_checkpoint_config
                .checkpoint_path
                .clone()
                .unwrap_or_else(|| config.db_checkpoint_path()),
        );
        let db_checkpoint_config = if config.db_checkpoint_config.checkpoint_path.is_none() {
            DBCheckpointConfig {
                checkpoint_path,
                perform_db_checkpoints_at_epoch_end: if state_snapshot_enabled {
                    true
                } else {
                    config
                        .db_checkpoint_config
                        .perform_db_checkpoints_at_epoch_end
                },
                ..config.db_checkpoint_config.clone()
            }
        } else {
            config.db_checkpoint_config.clone()
        };

        match (
            db_checkpoint_config.object_store_config.as_ref(),
            state_snapshot_enabled,
        ) {
            // If db checkpoint config object store not specified but
            // state snapshot object store is specified, create handler
            // anyway for marking db checkpoints as completed so that they
            // can be uploaded as state snapshots.
            (None, false) => Ok((db_checkpoint_config, None)),
            (_, _) => {
                let handler = DBCheckpointHandler::new(
                    &db_checkpoint_config.checkpoint_path.clone().unwrap(),
                    db_checkpoint_config.object_store_config.as_ref(),
                    60,
                    db_checkpoint_config
                        .prune_and_compact_before_upload
                        .unwrap_or(true),
                    config.indirect_objects_threshold,
                    config.authority_store_pruning_config,
                    prometheus_registry,
                    state_snapshot_enabled,
                )?;
                Ok((
                    db_checkpoint_config,
                    Some(DBCheckpointHandler::start(handler)),
                ))
            }
        }
    }

    fn create_p2p_network(
        config: &NodeConfig,
        state_sync_store: RocksDbStore,
        chain_identifier: ChainIdentifier,
        trusted_peer_change_rx: watch::Receiver<TrustedPeerChangeEvent>,
        archive_readers: ArchiveReaderBalancer,
        prometheus_registry: &Registry,
    ) -> Result<(Network, discovery::Handle, state_sync::Handle)> {
        let (state_sync, state_sync_server) = state_sync::Builder::new()
            .config(config.p2p_config.state_sync.clone().unwrap_or_default())
            .store(state_sync_store)
            .archive_readers(archive_readers)
            .with_metrics(prometheus_registry)
            .build();

        let (discovery, discovery_server) = discovery::Builder::new(trusted_peer_change_rx)
            .config(config.p2p_config.clone())
            .build();

        let p2p_network = {
            let routes = anemo::Router::new()
                .add_rpc_service(discovery_server)
                .add_rpc_service(state_sync_server);

            let inbound_network_metrics =
                NetworkMetrics::new("scalar", "inbound", prometheus_registry);
            let outbound_network_metrics =
                NetworkMetrics::new("scalar", "outbound", prometheus_registry);

            let service = ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_server_errors()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN)),
                )
                .layer(CallbackLayer::new(MetricsMakeCallbackHandler::new(
                    Arc::new(inbound_network_metrics),
                    config.p2p_config.excessive_message_size(),
                )))
                .service(routes);

            let outbound_layer = ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_client_and_server_errors()
                        .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                        .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN)),
                )
                .layer(CallbackLayer::new(MetricsMakeCallbackHandler::new(
                    Arc::new(outbound_network_metrics),
                    config.p2p_config.excessive_message_size(),
                )))
                .into_inner();

            let mut anemo_config = config.p2p_config.anemo_config.clone().unwrap_or_default();
            // Set the max_frame_size to be 1 GB to work around the issue of there being too many
            // staking events in the epoch change txn.
            anemo_config.max_frame_size = Some(1 << 30);

            // Set a higher default value for socket send/receive buffers if not already
            // configured.
            let mut quic_config = anemo_config.quic.unwrap_or_default();
            if quic_config.socket_send_buffer_size.is_none() {
                quic_config.socket_send_buffer_size = Some(20 << 20);
            }
            if quic_config.socket_receive_buffer_size.is_none() {
                quic_config.socket_receive_buffer_size = Some(20 << 20);
            }
            quic_config.allow_failed_socket_buffer_size_setting = true;

            // Set high-performance defaults for quinn transport.
            // With 200MiB buffer size and ~500ms RTT, max throughput ~400MiB/s.
            if quic_config.stream_receive_window.is_none() {
                quic_config.stream_receive_window = Some(100 << 20);
            }
            if quic_config.receive_window.is_none() {
                quic_config.receive_window = Some(200 << 20);
            }
            if quic_config.send_window.is_none() {
                quic_config.send_window = Some(200 << 20);
            }
            if quic_config.crypto_buffer_size.is_none() {
                quic_config.crypto_buffer_size = Some(1 << 20);
            }
            if quic_config.max_idle_timeout_ms.is_none() {
                quic_config.max_idle_timeout_ms = Some(30_000);
            }
            if quic_config.keep_alive_interval_ms.is_none() {
                quic_config.keep_alive_interval_ms = Some(5_000);
            }
            anemo_config.quic = Some(quic_config);

            let server_name = format!("scalar-{}", chain_identifier);
            let network = Network::bind(config.p2p_config.listen_address)
                .server_name(&server_name)
                .private_key(config.network_key_pair().copy().private().0.to_bytes())
                .config(anemo_config)
                .outbound_request_layer(outbound_layer)
                .start(service)?;
            info!(
                server_name = server_name,
                "P2p network started on {}",
                network.local_addr()
            );

            network
        };

        let discovery_handle = discovery.start(p2p_network.clone());
        let state_sync_handle = state_sync.start(p2p_network.clone());

        Ok((p2p_network, discovery_handle, state_sync_handle))
    }
    pub fn state(&self) -> Arc<AuthorityState> {
        self.state.clone()
    }
    // Init reconfig process by starting to reject user certs
    pub async fn close_epoch(&self, epoch_store: &Arc<AuthorityPerEpochStore>) -> SuiResult {
        info!("close_epoch (current epoch = {})", epoch_store.epoch());
        self.validator_node
            .lock()
            .await
            .consensus_adapter
            .close_epoch(epoch_store);
        Ok(())
    }
    pub fn clear_override_protocol_upgrade_buffer_stake(&self, epoch: EpochId) -> SuiResult {
        self.state
            .clear_override_protocol_upgrade_buffer_stake(epoch)
    }
    pub fn set_override_protocol_upgrade_buffer_stake(
        &self,
        epoch: EpochId,
        buffer_stake_bps: u64,
    ) -> SuiResult {
        self.state
            .set_override_protocol_upgrade_buffer_stake(epoch, buffer_stake_bps)
    }
}
