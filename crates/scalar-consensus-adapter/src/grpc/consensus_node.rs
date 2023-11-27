use std::net::SocketAddr;

use super::ConsensusService;
use crate::api::ConsensusMetrics;
use crate::proto::consensus_api_server::{ConsensusApi, ConsensusApiServer};
use crate::types::EthTransaction;
use anyhow::{Error, Result};
use scalar_core::authority::AuthorityState;
use scalar_core::authority_client::NetworkAuthorityClient;
use scalar_core::transaction_orchestrator::TransactiondOrchestrator;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, watch, Mutex, RwLock},
    task::JoinHandle,
};
use tonic::transport::Server;
pub struct ConsensusNodeInner {
    state: Arc<AuthorityState>,
    transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
    metrics: Arc<ConsensusMetrics>,
}

impl ConsensusNodeInner {
    async fn start(
        &mut self,
        addr: SocketAddr,
        rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
    ) -> Result<()> {
        // let (tx_eth_transaction, rx_eth_transaction) = mpsc::unbounded_channel();
        let consensus_service = ConsensusService::new(
            self.state.clone(),
            self.transaction_orchestrator.clone(),
            rx_ready_certificates,
            self.metrics.clone(),
        );
        Server::builder()
            .add_service(ConsensusApiServer::new(consensus_service))
            .serve(addr)
            // .serve_with_shutdown(
            //     grpc_addr.to_socket_addrs().unwrap().next().unwrap(),
            //     async {
            //         let _ = rx_shutdown.receiver.recv().await;
            //     },
            // )
            .await
            .unwrap();
        Ok(())
    }
}
pub struct ConsensusNode {
    internal: Arc<RwLock<ConsensusNodeInner>>,
}
impl ConsensusNode {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_orchestrator: Arc<TransactiondOrchestrator<NetworkAuthorityClient>>,
        metrics: Arc<ConsensusMetrics>,
    ) -> Self {
        let internal = ConsensusNodeInner {
            state,
            transaction_orchestrator,
            metrics,
        };
        let internal = Arc::new(RwLock::new(internal));
        ConsensusNode { internal }
    }
    pub async fn start(
        &self,
        grpc_address: SocketAddr,
        rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
    ) -> Result<()> {
        let mut guard = self.internal.write().await;
        guard.start(grpc_address, rx_ready_certificates).await
    }
}
