use std::net::SocketAddr;

use super::ConsensusService;
use crate::proto::consensus_api_server::{ConsensusApi, ConsensusApiServer};
use crate::types::EthTransaction;
use anyhow::{Error, Result};
use std::sync::Arc;
use tokio::{
    sync::{mpsc, watch, Mutex, RwLock},
    task::JoinHandle,
};
use tonic::transport::Server;
pub struct ConsensusNodeInner {
    tx_eth_transaction: mpsc::UnboundedSender<EthTransaction>,
}

impl ConsensusNodeInner {
    async fn start(&mut self, addr: SocketAddr) -> Result<Option<JoinHandle<()>>> {
        self.spawn(addr).await
    }
    async fn spawn(&mut self, addr: SocketAddr) -> Result<Option<JoinHandle<()>>> {
        let consensus_service = ConsensusService::new(self.tx_eth_transaction.clone());
        let handle = tokio::spawn(async move {
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
        });
        Ok(Some(handle))
    }
}
pub struct ConsensusNode {
    internal: Arc<RwLock<ConsensusNodeInner>>,
}
impl ConsensusNode {
    pub fn new(tx_eth_transaction: mpsc::UnboundedSender<EthTransaction>) -> Self {
        let internal = ConsensusNodeInner { tx_eth_transaction };
        let internal = Arc::new(RwLock::new(internal));
        ConsensusNode { internal }
    }
    pub async fn spawn(
        grpc_address: &SocketAddr,
        tx_eth_transaction: mpsc::UnboundedSender<EthTransaction>,
    ) -> Result<Option<tokio::task::JoinHandle<()>>> {
        let consensus = Self::new(tx_eth_transaction);
        let mut guard = consensus.internal.write().await;
        guard.start(grpc_address.clone()).await
    }
}
