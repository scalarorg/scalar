// Copyright (c) Scalar, org.
// SPDX-License-Identifier: Apache-2.0
/*
 * Consensus node only
 */

use crate::metrics::{GrpcMetrics, SuiNodeMetrics};
use anyhow::{anyhow, Result};
use arc_swap::ArcSwap;
use fastcrypto_zkp::bn254::zk_login::JwkId;
use fastcrypto_zkp::bn254::zk_login::OIDCProvider;
use fastcrypto_zkp::bn254::zk_login::JWK;
use futures::TryFutureExt;
use mysten_metrics::{spawn_monitored_task, RegistryService};
use mysten_network::server::ServerBuilder;
use narwhal_worker::TrivialTransactionValidator;
use prometheus::Registry;
use scalar_consensus_common::ConsensusApiServer;
use scalar_core::authority::authority_per_epoch_store::AuthorityPerEpochStore;
use scalar_core::authority::AuthorityState;
use scalar_core::authority_server::{ValidatorService, ValidatorServiceMetrics};
use scalar_core::checkpoints::{
    CheckpointMetrics, CheckpointService, CheckpointStore, SendCheckpointToStateSync,
    SubmitCheckpointToConsensus,
};
use scalar_core::epoch::data_removal::EpochDataRemover;
use scalar_core::state_accumulator::StateAccumulator;
use scalar_core::{
    consensus_adapter::{
        ConnectionMonitorStatus, ConsensusAdapter, ConsensusAdapterMetrics, LazyNarwhalClient,
        SubmitToConsensus,
    },
    consensus_handler::ConsensusHandlerInitializer,
    consensus_manager::{ConsensusManager, ConsensusManagerTrait},
    consensus_service::ConsensusService,
    consensus_throughput_calculator::{
        ConsensusThroughputCalculator, ConsensusThroughputProfiler, ThroughputProfileRanges,
    },
    mysticeti_adapter::LazyMysticetiClient,
    scalar_validator::{ScalarTxValidator, ScalarTxValidatorMetrics},
};
use std::collections::BTreeSet;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use sui_config::{node::ConsensusProtocol, ConsensusConfig, NodeConfig};
use sui_network::{api::ValidatorServer, state_sync};
use sui_protocol_config::ProtocolConfig;
use sui_types::error::SuiError;
use sui_types::error::SuiResult;
use sui_types::messages_consensus::check_total_jwk_size;
use sui_types::messages_consensus::ConsensusTransaction;
use sui_types::sui_system_state::epoch_start_sui_system_state::EpochStartSystemStateTrait;
use sui_types::{base_types::AuthorityName, committee::Committee};
use tap::tap::TapFallible;
use tokio::{sync::watch, task::JoinHandle};
use tracing::{debug, error, warn};
use tracing::{error_span, info, Instrument};

static MAX_JWK_KEYS_PER_FETCH: usize = 100;
pub struct ValidatorNode {
    validator_server_handle: JoinHandle<Result<()>>,
    consensus_manager: ConsensusManager,
    consensus_epoch_data_remover: EpochDataRemover,
    pub consensus_adapter: Arc<ConsensusAdapter>,
    // dropping this will eventually stop checkpoint tasks. The receiver side of this channel
    // is copied into each checkpoint service task, and they are listening to any change to this
    // channel. When the sender is dropped, a change is triggered and those tasks will exit.
    checkpoint_service_exit: watch::Sender<()>,
    checkpoint_metrics: Arc<CheckpointMetrics>,
    sui_tx_validator_metrics: Arc<ScalarTxValidatorMetrics>,
}

impl ValidatorNode {
    pub async fn start(
        config: &NodeConfig,
        state: Arc<AuthorityState>,
        committee: Arc<Committee>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        checkpoint_store: Arc<CheckpointStore>,
        state_sync_handle: state_sync::Handle,
        accumulator: Arc<StateAccumulator>,
        connection_monitor_status: Arc<ConnectionMonitorStatus>,
        registry_service: &RegistryService,
        sui_node_metrics: Arc<SuiNodeMetrics>,
    ) -> Result<ValidatorNode> {
        let consensus_config = config
            .consensus_config()
            .ok_or_else(|| anyhow!("Validator is missing consensus config"))?;
        let (consensus_adapter, consensus_manager) = match consensus_config.protocol {
            ConsensusProtocol::Narwhal => {
                let consensus_adapter = Arc::new(Self::construct_consensus_adapter(
                    &committee,
                    consensus_config,
                    state.name,
                    connection_monitor_status.clone(),
                    &registry_service.default_registry(),
                    epoch_store.protocol_config().clone(),
                    Arc::new(LazyNarwhalClient::new(
                        consensus_config.address().to_owned(),
                    )),
                ));
                let consensus_manager =
                    ConsensusManager::new_narwhal(config, consensus_config, registry_service);
                (consensus_adapter, consensus_manager)
            }
            ConsensusProtocol::Mysticeti => {
                let client = Arc::new(LazyMysticetiClient::new());

                let consensus_adapter = Arc::new(Self::construct_consensus_adapter(
                    &committee,
                    consensus_config,
                    state.name,
                    connection_monitor_status.clone(),
                    &registry_service.default_registry(),
                    epoch_store.protocol_config().clone(),
                    client.clone(),
                ));
                let consensus_manager = ConsensusManager::new_mysticeti(
                    config,
                    consensus_config,
                    registry_service,
                    client,
                );
                (consensus_adapter, consensus_manager)
            }
        };
        let mut consensus_epoch_data_remover =
            EpochDataRemover::new(consensus_manager.get_storage_base_path());

        // This only gets started up once, not on every epoch. (Make call to remove every epoch.)
        consensus_epoch_data_remover.run().await;

        let checkpoint_metrics = CheckpointMetrics::new(&registry_service.default_registry());
        let sui_tx_validator_metrics =
            ScalarTxValidatorMetrics::new(&registry_service.default_registry());

        let validator_server_handle = Self::start_grpc_validator_service(
            config,
            state.clone(),
            consensus_adapter.clone(),
            epoch_store.clone(),
            &registry_service.default_registry(),
        )
        .await?;

        Self::start_epoch_specific_validator_components(
            config,
            state.clone(),
            consensus_adapter,
            checkpoint_store,
            epoch_store,
            state_sync_handle,
            consensus_manager,
            consensus_epoch_data_remover,
            accumulator,
            validator_server_handle,
            checkpoint_metrics,
            sui_node_metrics,
            sui_tx_validator_metrics,
        )
        .await
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
    async fn start_epoch_specific_validator_components(
        config: &NodeConfig,
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        checkpoint_store: Arc<CheckpointStore>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        state_sync_handle: state_sync::Handle,
        consensus_manager: ConsensusManager,
        consensus_epoch_data_remover: EpochDataRemover,
        accumulator: Arc<StateAccumulator>,
        validator_server_handle: JoinHandle<Result<()>>,
        checkpoint_metrics: Arc<CheckpointMetrics>,
        sui_node_metrics: Arc<SuiNodeMetrics>,
        sui_tx_validator_metrics: Arc<ScalarTxValidatorMetrics>,
    ) -> Result<ValidatorNode> {
        let (checkpoint_service, checkpoint_service_exit) = Self::start_checkpoint_service(
            config,
            consensus_adapter.clone(),
            checkpoint_store,
            epoch_store.clone(),
            state.clone(),
            state_sync_handle,
            accumulator,
            checkpoint_metrics.clone(),
        );

        // create a new map that gets injected into both the consensus handler and the consensus adapter
        // the consensus handler will write values forwarded from consensus, and the consensus adapter
        // will read the values to make decisions about which validator submits a transaction to consensus
        let low_scoring_authorities = Arc::new(ArcSwap::new(Arc::new(HashMap::new())));

        consensus_adapter.swap_low_scoring_authorities(low_scoring_authorities.clone());

        let throughput_calculator = Arc::new(ConsensusThroughputCalculator::new(
            None,
            state.metrics.clone(),
        ));

        let throughput_profiler = Arc::new(ConsensusThroughputProfiler::new(
            throughput_calculator.clone(),
            None,
            None,
            state.metrics.clone(),
            ThroughputProfileRanges::from_chain(epoch_store.get_chain_identifier()),
        ));

        consensus_adapter.swap_throughput_profiler(throughput_profiler);

        let consensus_handler_initializer = ConsensusHandlerInitializer::new(
            state.clone(),
            checkpoint_service.clone(),
            epoch_store.clone(),
            low_scoring_authorities,
            throughput_calculator,
        );
        // Scalar Todo: Disable TxValidator for testing
        consensus_manager
            .start(
                config,
                epoch_store.clone(),
                consensus_handler_initializer,
                ScalarTxValidator::new(
                    epoch_store.clone(),
                    checkpoint_service.clone(),
                    state.transaction_manager().clone(),
                    sui_tx_validator_metrics.clone(),
                ),
            )
            .await;

        if epoch_store.authenticator_state_enabled() {
            Self::start_jwk_updater(
                config,
                sui_node_metrics,
                state.name,
                epoch_store.clone(),
                consensus_adapter.clone(),
            );
        }

        Ok(ValidatorNode {
            validator_server_handle,
            consensus_manager,
            consensus_epoch_data_remover,
            consensus_adapter,
            checkpoint_service_exit,
            checkpoint_metrics,
            sui_tx_validator_metrics,
        })
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
            validator_service.clone(),
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
impl ValidatorNode {
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
impl ValidatorNode {
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

#[cfg(not(test))]
fn max_tx_per_checkpoint(protocol_config: &ProtocolConfig) -> usize {
    protocol_config.max_transactions_per_checkpoint() as usize
}

#[cfg(test)]
fn max_tx_per_checkpoint(_: &ProtocolConfig) -> usize {
    2
}
