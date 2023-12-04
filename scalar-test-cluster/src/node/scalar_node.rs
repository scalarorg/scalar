use anyhow::Result;
use mysten_metrics::{spawn_monitored_task, RegistryService};
use prometheus::Registry;
use std::{fmt, sync::Arc};
use tokio::runtime::Handle;
use tracing::info;

use crate::{
    config::node_config::ScalarNodeConfig,
    core::{authority::AuthorityState, grpc_server::build_grpc_server},
};
pub struct ScalarNode {
    state: Arc<AuthorityState>,
}

impl fmt::Debug for ScalarNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ScalarNode")
            .field("name", &self.state.name.concise())
            .finish()
    }
}

static MAX_JWK_KEYS_PER_FETCH: usize = 100;

impl ScalarNode {
    pub async fn start(
        config: &ScalarNodeConfig,
        registry_service: RegistryService,
        custom_rpc_runtime: Option<Handle>,
        consensus_runtime: Option<Handle>,
    ) -> Result<Arc<ScalarNode>> {
        Self::start_async(
            config,
            registry_service,
            custom_rpc_runtime,
            consensus_runtime,
        )
        .await
    }
    pub async fn start_async(
        config: &ScalarNodeConfig,
        registry_service: RegistryService,
        custom_rpc_runtime: Option<Handle>,
        consensus_runtime: Option<Handle>,
    ) -> Result<Arc<ScalarNode>> {
        let state = AuthorityState::new(
            config.node_config.protocol_public_key(),
            // secret,
            // config.supported_protocol_versions.unwrap(),
            // store.clone(),
            // epoch_store.clone(),
            // committee_store.clone(),
            // index_store.clone(),
            // checkpoint_store.clone(),
            // &prometheus_registry,
            // pruning_config,
            // genesis.objects(),
            // &db_checkpoint_config,
            config.node_config.expensive_safety_check_config.clone(),
            config.node_config.transaction_deny_config.clone(),
            config.node_config.certificate_deny_config.clone(),
            config.node_config.indirect_objects_threshold,
            config.node_config.state_debug_dump_config.clone(),
            config.node_config.overload_threshold_config.clone(),
            // archive_readers,
        )
        .await;

        let grpc_handle = build_grpc_server(
            state.clone(),
            &transaction_orchestrator,
            rx_ready_certificates,
            &config,
            &prometheus_registry,
            consensus_runtime,
        )
        .await?;
        let node = Self { state };
        info!("ScalarNode started!");
        let node = Arc::new(node);
        let node_copy = node.clone();
        // spawn_monitored_task!(async move { Self::monitor_reconfiguration(node_copy).await });

        Ok(node)
    }
}
