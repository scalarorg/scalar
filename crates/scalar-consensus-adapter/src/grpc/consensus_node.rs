use std::net::SocketAddr;

use super::{ConsensusService, ListenerCollection};
use crate::api::ConsensusMetrics;
use anyhow::{Error, Result};
use scalar_consensus_adapter_common::proto::consensus_api_server::ConsensusApiServer;
use scalar_consensus_adapter_common::proto::ConsensusTransactionOut;
use scalar_core::authority::{AuthorityState, CommitedCertificates};
use scalar_core::authority_client::NetworkAuthorityClient;
use scalar_core::transaction_orchestrator::TransactiondOrchestrator;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::RwLock;
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
        let listeners = Arc::new(RwLock::new(vec![]));
        let consensus_service = ConsensusService::new(
            self.state.clone(),
            self.transaction_orchestrator.clone(),
            listeners.clone(),
            self.metrics.clone(),
        );
        /*
         * 231128 Taivv
         * Cùng với grpc server sẽ chạy 1 Tokio thread để nhận thông tin về commited Certificateds
         * thông qua rx_ready_certificates và gửi tới các client đã có kết nối
         * Start ready_sertificate notifier
         */
        self.start_notifier(rx_ready_certificates, listeners).await;
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
    pub async fn start_notifier(
        &self,
        mut rx_ready_certificates: UnboundedReceiver<CommitedCertificates>,
        consensus_listeners: Arc<RwLock<ListenerCollection>>,
    ) {
        let _handle = tokio::spawn(async move {
            let listener = consensus_listeners;
            while let Some((cert, ef_digest)) = rx_ready_certificates.recv().await {
                let guard = listener.write().await;
                for sender in guard.iter() {
                    // 231128 - TaiVV
                    // Scalar TODO: Add converter here
                    let consensus_out = ConsensusTransactionOut {
                        payload: vec![0; 32],
                    };
                    let _res = sender.send(Ok(consensus_out));
                }
            }
        });
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
