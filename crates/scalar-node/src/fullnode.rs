// Copyright (c) Scalar, org.
// SPDX-License-Identifier: Apache-2.0
/*
 * Full node only
 */

use crate::metrics::{GrpcMetrics, SuiNodeMetrics};
use anemo::Network;
use anemo_tower::callback::CallbackLayer;
use anemo_tower::trace::DefaultMakeSpan;
use anemo_tower::trace::DefaultOnFailure;
use anemo_tower::trace::TraceLayer;
use anyhow::{anyhow, Result};
use arc_swap::ArcSwap;
use fastcrypto_zkp::bn254::zk_login::{JwkId, OIDCProvider, JWK};
use futures::TryFutureExt;
use mysten_metrics::{spawn_monitored_task, RegistryService};
use mysten_network::server::ServerBuilder;
use narwhal_network::metrics::{
    MetricsMakeCallbackHandler, NetworkConnectionMetrics, NetworkMetrics,
};
use prometheus::Registry;
use scalar_consensus_common::ConsensusApiServer;
use scalar_core::authority::authority_per_epoch_store::AuthorityPerEpochStore;
use scalar_core::authority::authority_store_tables::AuthorityPerpetualTables;
use scalar_core::authority::AuthorityState;
use scalar_core::authority::AuthorityStore;
use scalar_core::authority::CHAIN_IDENTIFIER;
use scalar_core::authority_client::NetworkAuthorityClient;
use scalar_core::authority_server::{ValidatorService, ValidatorServiceMetrics};
use scalar_core::checkpoints::{
    CheckpointMetrics, CheckpointService, CheckpointStore, SendCheckpointToStateSync,
    SubmitCheckpointToConsensus,
};
use scalar_core::consensus_service::ConsensusService;
use scalar_core::db_checkpoint_handler::DBCheckpointHandler;
use scalar_core::epoch::committee_store::CommitteeStore;
use scalar_core::epoch::data_removal::EpochDataRemover;
use scalar_core::epoch::epoch_metrics::EpochMetrics;
use scalar_core::module_cache_metrics::ResolverMetrics;
use scalar_core::mysticeti_adapter::LazyMysticetiClient;
use scalar_core::signature_verifier::SignatureVerifierMetrics;
use scalar_core::state_accumulator::StateAccumulator;
use scalar_core::storage::RocksDbStore;
use scalar_core::transaction_orchestrator::TransactiondOrchestrator;
use scalar_core::verify_indexes;
use scalar_core::{
    consensus_adapter::{
        ConnectionMonitorStatus, ConsensusAdapter, ConsensusAdapterMetrics, LazyNarwhalClient,
        SubmitToConsensus,
    },
    consensus_handler::ConsensusHandlerInitializer,
    consensus_manager::{ConsensusManager, ConsensusManagerTrait},
    consensus_throughput_calculator::{
        ConsensusThroughputCalculator, ConsensusThroughputProfiler, ThroughputProfileRanges,
    },
};
use scalar_kvstore::writer::setup_key_value_store_uploader;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::{fmt, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
use sui_archival::reader::ArchiveReaderBalancer;
use sui_archival::writer::ArchiveWriter;
use sui_config::node::ConsensusProtocol;
use sui_config::node::DBCheckpointConfig;
use sui_config::node_config_metrics::NodeConfigMetrics;
use sui_config::ConsensusConfig;
use sui_config::NodeConfig;
use sui_json_rpc::{
    coin_api::CoinReadApi, governance_api::GovernanceReadApi, indexer_api::IndexerApi,
    move_utils::MoveUtils, read_api::ReadApi, transaction_builder_api::TransactionBuilderApi,
    transaction_execution_api::TransactionExecutionApi, JsonRpcServerBuilder,
};
use sui_json_rpc_api::JsonRpcMetrics;
use sui_macros::replay_log;
use sui_network::api::ValidatorServer;
use sui_network::discovery;
use sui_network::discovery::TrustedPeerChangeEvent;
use sui_network::state_sync;
use sui_protocol_config::Chain;
use sui_protocol_config::{ProtocolConfig, SupportedProtocolVersions};
use sui_snapshot::uploader::StateSnapshotUploader;
use sui_storage::{
    http_key_value_store::HttpKVStore,
    key_value_store::{FallbackTransactionKVStore, TransactionKeyValueStore},
    key_value_store_metrics::KeyValueStoreMetrics,
    object_store::{ObjectStoreConfig, ObjectStoreType},
    FileCompression, IndexStore, StorageFormat,
};
use sui_types::base_types::AuthorityName;
use sui_types::committee::Committee;
use sui_types::committee::EpochId;
use sui_types::crypto::KeypairTraits;
use sui_types::digests::ChainIdentifier;
use sui_types::error::SuiError;
use sui_types::error::SuiResult;
use sui_types::messages_consensus::{
    check_total_jwk_size, AuthorityCapabilities, ConsensusTransaction,
};
use sui_types::sui_system_state::{
    epoch_start_sui_system_state::{EpochStartSystemState, EpochStartSystemStateTrait},
    SuiSystemState,
};
use tap::TapFallible;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tracing::Instrument;
use tracing::{debug, error, error_span, info, warn};
use typed_store::rocks::default_db_options;
use typed_store::DBMetrics;

static MAX_JWK_KEYS_PER_FETCH: usize = 100;
pub struct FullNode {
    config: NodeConfig,
    /// The http server responsible for serving JSON-RPC as well as the experimental rest service
    _http_server: Option<tokio::task::JoinHandle<()>>,
    state: Arc<AuthorityState>,
    transaction_orchestrator: Option<Arc<TransactiondOrchestrator<NetworkAuthorityClient>>>,
    registry_service: RegistryService,
    metrics: Arc<SuiNodeMetrics>,

    _discovery: discovery::Handle,
    state_sync: state_sync::Handle,
    checkpoint_store: Arc<CheckpointStore>,
    accumulator: Arc<StateAccumulator>,
    connection_monitor_status: Arc<ConnectionMonitorStatus>,

    /// Broadcast channel to send the starting system state for the next epoch.
    end_of_epoch_channel: broadcast::Sender<SuiSystemState>,

    /// Broadcast channel to notify state-sync for new validator peers.
    trusted_peer_change_tx: watch::Sender<TrustedPeerChangeEvent>,

    _db_checkpoint_handle: Option<tokio::sync::broadcast::Sender<()>>,

    #[cfg(msim)]
    sim_state: SimState,

    _state_archive_handle: Option<broadcast::Sender<()>>,

    _state_snapshot_uploader_handle: Option<broadcast::Sender<()>>,
    _kv_store_uploader_handle: Option<oneshot::Sender<()>>,
}

impl FullNode {
    pub async fn start(
        config: &NodeConfig,
        registry_service: RegistryService,
        custom_rpc_runtime: Option<Handle>,
    ) -> Result<Arc<FullNode>> {
        Self::start_async(config, registry_service, custom_rpc_runtime).await
    }

    pub async fn start_async(
        config: &NodeConfig,
        registry_service: RegistryService,
        custom_rpc_runtime: Option<Handle>,
    ) -> Result<Arc<FullNode>> {
        let is_full_node = true;
        NodeConfigMetrics::new(&registry_service.default_registry()).record_metrics(config);
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
        replay_log!(
            "Beginning replay run. Epoch: {:?}, Protocol config: {:?}",
            epoch_store.epoch(),
            epoch_store.protocol_config()
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

        let index_store = if is_full_node && config.enable_index_processing {
            Some(Arc::new(IndexStore::new(
                config.db_path().join("indexes"),
                &prometheus_registry,
                epoch_store
                    .protocol_config()
                    .max_move_identifier_len_as_option(),
            )))
        } else {
            None
        };

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
        // We must explicitly send this instead of relying on the initial value to trigger
        // watch value change, so that state-sync is able to process it.
        send_trusted_peer_change(
            &config,
            &trusted_peer_change_tx,
            epoch_store.epoch_start_state(),
        )
        .expect("Initial trusted peers must be set");

        // Start uploading transactions/events to remote key value store
        let kv_store_uploader_handle = setup_key_value_store_uploader(
            state_sync_store.clone(),
            &config.transaction_kv_store_write_config,
            &prometheus_registry,
        )
        .await?;

        // Start archiving local state to remote store
        let state_archive_handle =
            Self::start_state_archival(&config, &prometheus_registry, state_sync_store).await?;

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

        // ensure genesis txn was executed
        if epoch_store.epoch() == 0 {
            let txn = &genesis.transaction();
            let span = error_span!("genesis_txn", tx_digest = ?txn.digest());
            let transaction =
                sui_types::executable_transaction::VerifiedExecutableTransaction::new_unchecked(
                    sui_types::executable_transaction::ExecutableTransaction::new_from_data_and_sig(
                        genesis.transaction().data().clone(),
                        sui_types::executable_transaction::CertificateProof::Checkpoint(0, 0),
                    ),
                );
            state
                .try_execute_immediately(&transaction, None, &epoch_store)
                .instrument(span)
                .await
                .unwrap();
        }

        if config
            .expensive_safety_check_config
            .enable_secondary_index_checks()
        {
            if let Some(indexes) = state.indexes.clone() {
                verify_indexes::verify_indexes(state.database.clone(), indexes)
                    .expect("secondary indexes are inconsistent");
            }
        }

        let (end_of_epoch_channel, end_of_epoch_receiver) =
            broadcast::channel(config.end_of_epoch_broadcast_channel_capacity);

        let transaction_orchestrator = if is_full_node {
            Some(Arc::new(
                TransactiondOrchestrator::new_with_network_clients(
                    state.clone(),
                    end_of_epoch_receiver,
                    &config.db_path(),
                    &prometheus_registry,
                )?,
            ))
        } else {
            None
        };

        let http_server = build_http_server(
            state.clone(),
            &transaction_orchestrator.clone(),
            &config,
            &prometheus_registry,
            custom_rpc_runtime,
        )?;

        let accumulator = Arc::new(StateAccumulator::new(store));

        let authority_names_to_peer_ids = epoch_store
            .epoch_start_state()
            .get_authority_names_to_peer_ids();

        let network_connection_metrics =
            NetworkConnectionMetrics::new("sui", &registry_service.default_registry());

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
        /*
         * 2023 Dec 28
         * Scalar: TaiVV
         * Construct consensus adapter for fullnode
         */
        let consensus_config = config
            .consensus_config()
            .ok_or_else(|| anyhow!("Validator is missing consensus config"))?;
        let client: Arc<dyn SubmitToConsensus> = match consensus_config.protocol {
            ConsensusProtocol::Narwhal => Arc::new(LazyNarwhalClient::new(
                consensus_config.address().to_owned(),
            )),
            ConsensusProtocol::Mysticeti => Arc::new(LazyMysticetiClient::new()),
        };
        let consensus_adapter = Arc::new(Self::construct_consensus_adapter(
            &committee,
            consensus_config,
            state.name,
            connection_monitor_status.clone(),
            &registry_service.default_registry(),
            epoch_store.protocol_config().clone(),
            client,
        ));
        let node = Self {
            config,
            _http_server: http_server,
            state,
            transaction_orchestrator,
            registry_service,
            metrics: sui_node_metrics,

            _discovery: discovery_handle,
            state_sync: state_sync_handle,
            checkpoint_store,
            accumulator,
            end_of_epoch_channel,
            connection_monitor_status,
            trusted_peer_change_tx,

            _db_checkpoint_handle: db_checkpoint_handle,

            #[cfg(msim)]
            sim_state: Default::default(),

            _state_archive_handle: state_archive_handle,
            _state_snapshot_uploader_handle: state_snapshot_handle,
            _kv_store_uploader_handle: kv_store_uploader_handle,
        };

        info!("FullNode started!");
        Ok(Arc::new(node))
    }
    pub fn state(&self) -> Arc<AuthorityState> {
        self.state.clone()
    }
    fn start_jwk_updater(
        config: &NodeConfig,
        metrics: Arc<SuiNodeMetrics>,
        authority: AuthorityName,
        epoch_store: Arc<AuthorityPerEpochStore>,
        consensus_adapter: Arc<ConsensusAdapter>,
    ) {
        let epoch = epoch_store.epoch();

        let supported_providers = config
            .zklogin_oauth_providers
            .get(&epoch_store.get_chain_identifier().chain())
            .unwrap_or(&BTreeSet::new())
            .iter()
            .map(|s| OIDCProvider::from_str(s).expect("Invalid provider string"))
            .collect::<Vec<_>>();

        let fetch_interval = Duration::from_secs(config.jwk_fetch_interval_seconds);

        info!(
            ?fetch_interval,
            "Starting JWK updater tasks with supported providers: {:?}", supported_providers
        );

        fn validate_jwk(
            metrics: &Arc<SuiNodeMetrics>,
            provider: &OIDCProvider,
            id: &JwkId,
            jwk: &JWK,
        ) -> bool {
            let Ok(iss_provider) = OIDCProvider::from_iss(&id.iss) else {
                warn!(
                    "JWK iss {:?} (retrieved from {:?}) is not a valid provider",
                    id.iss, provider
                );
                metrics
                    .invalid_jwks
                    .with_label_values(&[&provider.to_string()])
                    .inc();
                return false;
            };

            if iss_provider != *provider {
                warn!(
                    "JWK iss {:?} (retrieved from {:?}) does not match provider {:?}",
                    id.iss, provider, iss_provider
                );
                metrics
                    .invalid_jwks
                    .with_label_values(&[&provider.to_string()])
                    .inc();
                return false;
            }

            if !check_total_jwk_size(id, jwk) {
                warn!("JWK {:?} (retrieved from {:?}) is too large", id, provider);
                metrics
                    .invalid_jwks
                    .with_label_values(&[&provider.to_string()])
                    .inc();
                return false;
            }

            true
        }

        // metrics is:
        //  pub struct SuiNodeMetrics {
        //      pub jwk_requests: IntCounterVec,
        //      pub jwk_request_errors: IntCounterVec,
        //      pub total_jwks: IntCounterVec,
        //      pub unique_jwks: IntCounterVec,
        //  }

        for p in supported_providers.into_iter() {
            let provider_str = p.to_string();
            let epoch_store = epoch_store.clone();
            let consensus_adapter = consensus_adapter.clone();
            let metrics = metrics.clone();
            spawn_monitored_task!(epoch_store.clone().within_alive_epoch(
                async move {
                    // note: restart-safe de-duplication happens after consensus, this is
                    // just best-effort to reduce unneeded submissions.
                    let mut seen = HashSet::new();
                    loop {
                        info!("fetching JWK for provider {:?}", p);
                        metrics.jwk_requests.with_label_values(&[&provider_str]).inc();
                        match Self::fetch_jwks(authority, &p).await {
                            Err(e) => {
                                metrics.jwk_request_errors.with_label_values(&[&provider_str]).inc();
                                warn!("Error when fetching JWK for provider {:?} {:?}", p, e);
                                // Retry in 30 seconds
                                tokio::time::sleep(Duration::from_secs(30)).await;
                                continue;
                            }
                            Ok(mut keys) => {
                                metrics.total_jwks
                                    .with_label_values(&[&provider_str])
                                    .inc_by(keys.len() as u64);

                                keys.retain(|(id, jwk)| {
                                    validate_jwk(&metrics, &p, id, jwk) &&
                                    !epoch_store.jwk_active_in_current_epoch(id, jwk) &&
                                    seen.insert((id.clone(), jwk.clone()))
                                });

                                metrics.unique_jwks
                                    .with_label_values(&[&provider_str])
                                    .inc_by(keys.len() as u64);

                                // prevent oauth providers from sending too many keys,
                                // inadvertently or otherwise
                                if keys.len() > MAX_JWK_KEYS_PER_FETCH {
                                    warn!("Provider {:?} sent too many JWKs, only the first {} will be used", p, MAX_JWK_KEYS_PER_FETCH);
                                    keys.truncate(MAX_JWK_KEYS_PER_FETCH);
                                }

                                for (id, jwk) in keys.into_iter() {
                                    info!("Submitting JWK to consensus: {:?}", id);

                                    let txn = ConsensusTransaction::new_jwk_fetched(authority, id, jwk);
                                    consensus_adapter.submit(txn, None, &epoch_store)
                                        .tap_err(|e| warn!("Error when submitting JWKs to consensus {:?}", e))
                                        .ok();
                                }
                            }
                        }
                        tokio::time::sleep(fetch_interval).await;
                    }
                }
                .instrument(error_span!("jwk_updater_task", epoch)),
            ));
        }
    }
    pub fn subscribe_to_epoch_change(&self) -> broadcast::Receiver<SuiSystemState> {
        self.end_of_epoch_channel.subscribe()
    }

    pub fn current_epoch_for_testing(&self) -> EpochId {
        self.state.current_epoch_for_testing()
    }

    pub fn db_checkpoint_path(&self) -> PathBuf {
        self.config.db_checkpoint_path()
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

    async fn start_state_archival(
        config: &NodeConfig,
        prometheus_registry: &Registry,
        state_sync_store: RocksDbStore,
    ) -> Result<Option<tokio::sync::broadcast::Sender<()>>> {
        if let Some(remote_store_config) = &config.state_archive_write_config.object_store_config {
            let local_store_config = ObjectStoreConfig {
                object_store: Some(ObjectStoreType::File),
                directory: Some(config.archive_path()),
                ..Default::default()
            };
            let archive_writer = ArchiveWriter::new(
                local_store_config,
                remote_store_config.clone(),
                FileCompression::Zstd,
                StorageFormat::Blob,
                Duration::from_secs(600),
                256 * 1024 * 1024,
                prometheus_registry,
            )
            .await?;
            Ok(Some(archive_writer.start(state_sync_store).await?))
        } else {
            Ok(None)
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
                NetworkMetrics::new("sui", "inbound", prometheus_registry);
            let outbound_network_metrics =
                NetworkMetrics::new("sui", "outbound", prometheus_registry);

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

            let server_name = format!("sui-{}", chain_identifier);
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
    fn start_checkpoint_service(
        config: &NodeConfig,
        consensus_adapter: Arc<ConsensusAdapter>,
        checkpoint_store: Arc<CheckpointStore>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        state: Arc<AuthorityState>,
        state_sync_handle: state_sync::Handle,
        accumulator: Arc<StateAccumulator>,
        checkpoint_metrics: Arc<CheckpointMetrics>,
    ) -> (Arc<CheckpointService>, watch::Sender<()>) {
        let epoch_start_timestamp_ms = epoch_store.epoch_start_state().epoch_start_timestamp_ms();
        let epoch_duration_ms = epoch_store.epoch_start_state().epoch_duration_ms();

        debug!(
            "Starting checkpoint service with epoch start timestamp {}
            and epoch duration {}",
            epoch_start_timestamp_ms, epoch_duration_ms
        );

        let checkpoint_output = Box::new(SubmitCheckpointToConsensus {
            sender: consensus_adapter,
            signer: state.secret.clone(),
            authority: config.protocol_public_key(),
            next_reconfiguration_timestamp_ms: epoch_start_timestamp_ms
                .checked_add(epoch_duration_ms)
                .expect("Overflow calculating next_reconfiguration_timestamp_ms"),
            metrics: checkpoint_metrics.clone(),
        });

        let certified_checkpoint_output = SendCheckpointToStateSync::new(state_sync_handle);
        let max_tx_per_checkpoint = max_tx_per_checkpoint(epoch_store.protocol_config());
        let max_checkpoint_size_bytes =
            epoch_store.protocol_config().max_checkpoint_size_bytes() as usize;

        CheckpointService::spawn(
            state.clone(),
            checkpoint_store,
            epoch_store,
            Box::new(state.db()),
            accumulator,
            checkpoint_output,
            Box::new(certified_checkpoint_output),
            checkpoint_metrics,
            max_tx_per_checkpoint,
            max_checkpoint_size_bytes,
        )
    }

    fn construct_consensus_adapter(
        committee: &Committee,
        consensus_config: &ConsensusConfig,
        authority: AuthorityName,
        connection_monitor_status: Arc<ConnectionMonitorStatus>,
        prometheus_registry: &Registry,
        protocol_config: ProtocolConfig,
        consensus_client: Arc<dyn SubmitToConsensus>,
    ) -> ConsensusAdapter {
        let ca_metrics = ConsensusAdapterMetrics::new(prometheus_registry);
        // The consensus adapter allows the authority to send user certificates through consensus.

        ConsensusAdapter::new(
            consensus_client,
            authority,
            connection_monitor_status,
            consensus_config.max_pending_transactions(),
            consensus_config.max_pending_transactions() * 2 / committee.num_members(),
            consensus_config.max_submit_position,
            consensus_config.submit_delay_step_override(),
            ca_metrics,
            protocol_config,
        )
    }

    async fn start_grpc_validator_service(
        config: &NodeConfig,
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        prometheus_registry: &Registry,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let network_addr = config.network_address();
        info!("Start grpc validator service at {network_addr}");
        let validator_service = ValidatorService::new(
            state.clone(),
            consensus_adapter.clone(),
            Arc::new(ValidatorServiceMetrics::new(prometheus_registry)),
        );

        let mut server_conf = mysten_network::config::Config::new();
        server_conf.global_concurrency_limit = config.grpc_concurrency_limit;
        server_conf.load_shed = config.grpc_load_shed;
        let mut server_builder =
            ServerBuilder::from_config(&server_conf, GrpcMetrics::new(prometheus_registry));
        let consensus_service = ConsensusService::new(
            state,
            consensus_adapter,
            epoch_store,
            prometheus_registry,
        );
        server_builder = server_builder.add_service(ValidatorServer::new(validator_service));

        server_builder = server_builder.add_service(ConsensusApiServer::new(consensus_service));
        let server = server_builder
            .bind(config.network_address())
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        let local_addr = server.local_addr();
        info!("Listening to traffic on {local_addr}");
        let grpc_server = spawn_monitored_task!(server.serve().map_err(Into::into));

        Ok(grpc_server)
    }
}

#[cfg(not(msim))]
impl FullNode {
    async fn fetch_jwks(
        _authority: AuthorityName,
        provider: &OIDCProvider,
    ) -> SuiResult<Vec<(JwkId, JWK)>> {
        use fastcrypto_zkp::bn254::zk_login::fetch_jwks;
        let client = reqwest::Client::new();
        fetch_jwks(provider, &client)
            .await
            .map_err(|_| SuiError::JWKRetrievalError)
    }
}

#[cfg(msim)]
impl FullNode {
    pub fn get_sim_node_id(&self) -> sui_simulator::task::NodeId {
        self.sim_state.sim_node.id()
    }

    pub fn set_safe_mode_expected(&self, new_value: bool) {
        info!("Setting safe mode expected to {}", new_value);
        self.sim_state
            .sim_safe_mode_expected
            .store(new_value, Ordering::Relaxed);
    }

    #[allow(unused_variables)]
    async fn fetch_jwks(
        authority: AuthorityName,
        provider: &OIDCProvider,
    ) -> SuiResult<Vec<(JwkId, JWK)>> {
        get_jwk_injector()(authority, provider)
    }
}

/// Notify state-sync that a new list of trusted peers are now available.
fn send_trusted_peer_change(
    config: &NodeConfig,
    sender: &watch::Sender<TrustedPeerChangeEvent>,
    epoch_state_state: &EpochStartSystemState,
) -> Result<(), watch::error::SendError<TrustedPeerChangeEvent>> {
    sender
        .send(TrustedPeerChangeEvent {
            new_peers: epoch_state_state.get_validator_as_p2p_peers(config.protocol_public_key()),
        })
        .tap_err(|err| {
            warn!(
                "Failed to send validator peer information to state sync: {:?}",
                err
            );
        })
}

fn build_kv_store(
    state: &Arc<AuthorityState>,
    config: &NodeConfig,
    registry: &Registry,
) -> Result<Arc<TransactionKeyValueStore>> {
    let metrics = KeyValueStoreMetrics::new(registry);
    let db_store = TransactionKeyValueStore::new("rocksdb", metrics.clone(), state.clone());

    let base_url = &config.transaction_kv_store_read_config.base_url;

    if base_url.is_empty() {
        info!("no http kv store url provided, using local db only");
        return Ok(Arc::new(db_store));
    }

    let base_url: url::Url = base_url.parse().tap_err(|e| {
        error!(
            "failed to parse config.transaction_kv_store_config.base_url ({:?}) as url: {}",
            base_url, e
        )
    })?;

    let network_str = match state.get_chain_identifier().map(|c| c.chain()) {
        Some(Chain::Mainnet) => "/mainnet",
        Some(Chain::Testnet) => "/testnet",
        _ => {
            info!("using local db only for kv store for unknown chain");
            return Ok(Arc::new(db_store));
        }
    };

    let base_url = base_url.join(network_str)?.to_string();
    let http_store = HttpKVStore::new_kv(&base_url, metrics.clone())?;
    info!("using local key-value store with fallback to http key-value store");
    Ok(Arc::new(FallbackTransactionKVStore::new_kv(
        db_store,
        http_store,
        metrics,
        "json_rpc_fallback",
    )))
}

pub fn build_http_server(
    state: Arc<AuthorityState>,
    transaction_orchestrator: &Option<Arc<TransactiondOrchestrator<NetworkAuthorityClient>>>,
    config: &NodeConfig,
    prometheus_registry: &Registry,
    _custom_runtime: Option<Handle>,
) -> Result<Option<tokio::task::JoinHandle<()>>> {
    // Validators do not expose these APIs
    if config.consensus_config().is_some() {
        return Ok(None);
    }

    let mut router = axum::Router::new();

    let json_rpc_router = {
        let mut server = JsonRpcServerBuilder::new(env!("CARGO_PKG_VERSION"), prometheus_registry);

        let kv_store = build_kv_store(&state, config, prometheus_registry)?;

        let metrics = Arc::new(JsonRpcMetrics::new(prometheus_registry));
        server.register_module(ReadApi::new(
            state.clone(),
            kv_store.clone(),
            metrics.clone(),
        ))?;
        server.register_module(CoinReadApi::new(
            state.clone(),
            kv_store.clone(),
            metrics.clone(),
        ))?;
        server.register_module(TransactionBuilderApi::new(state.clone()))?;
        server.register_module(GovernanceReadApi::new(state.clone(), metrics.clone()))?;

        if let Some(transaction_orchestrator) = transaction_orchestrator {
            server.register_module(TransactionExecutionApi::new(
                state.clone(),
                transaction_orchestrator.clone(),
                metrics.clone(),
            ))?;
        }

        let name_service_config =
            if let (Some(package_address), Some(registry_id), Some(reverse_registry_id)) = (
                config.name_service_package_address,
                config.name_service_registry_id,
                config.name_service_reverse_registry_id,
            ) {
                sui_json_rpc::name_service::NameServiceConfig::new(
                    package_address,
                    registry_id,
                    reverse_registry_id,
                )
            } else {
                sui_json_rpc::name_service::NameServiceConfig::default()
            };

        server.register_module(IndexerApi::new(
            state.clone(),
            ReadApi::new(state.clone(), kv_store.clone(), metrics.clone()),
            kv_store,
            name_service_config,
            metrics,
            config.indexer_max_subscriptions,
        ))?;
        server.register_module(MoveUtils::new(state.clone()))?;

        server.to_router(None)?
    };

    router = router.merge(json_rpc_router);

    if config.enable_experimental_rest_api {
        let rest_router = sui_rest_api::rest_router(state);
        router = router.nest("/rest", rest_router);
    }

    let server = axum::Server::bind(&config.json_rpc_address).serve(router.into_make_service());

    let addr = server.local_addr();
    let handle = tokio::spawn(async move { server.await.unwrap() });

    info!(local_addr =? addr, "Sui JSON-RPC server listening on {addr}");

    Ok(Some(handle))
}

#[cfg(not(test))]
fn max_tx_per_checkpoint(protocol_config: &ProtocolConfig) -> usize {
    protocol_config.max_transactions_per_checkpoint() as usize
}

#[cfg(test)]
fn max_tx_per_checkpoint(_: &ProtocolConfig) -> usize {
    2
}
