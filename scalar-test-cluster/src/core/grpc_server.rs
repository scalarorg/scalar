use anyhow::anyhow;
use anyhow::Result;
use prometheus::Registry;
use std::sync::Arc;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;

use crate::config::node_config::NodeConfig;
use crate::config::node_config::ScalarNodeConfig;

pub async fn build_grpc_server(
    state: Arc<AuthorityState>,
    transaction_orchestrator: &Option<Arc<TransactiondOrchestrator<NetworkAuthorityClient>>>,
    rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
    config: &ScalarNodeConfig,
    prometheus_registry: &Registry,
    consensus_runtime: Option<Handle>,
) -> Result<Option<tokio::task::JoinHandle<()>>> {
    Ok(transaction_orchestrator.as_ref().map(|trans_orches| {
        let metrics = Arc::new(ConsensusMetrics::new(prometheus_registry));
        let consensus_node = ConsensusNode::new(state, trans_orches.clone(), metrics.clone());
        let grpc_address = config.grpc_consensus_address.clone();
        let handle = tokio::spawn(async move {
            consensus_node
                .start(grpc_address, rx_ready_certificates)
                .await
                .unwrap()
        });
        info!("Consensus Grpc server listenning on {:?}", &grpc_address);
        handle
    }))
    //Ok(Some(handle))
}
