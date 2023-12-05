use std::net::SocketAddr;

use crate::swarm::Swarm;
use jsonrpsee::{
    http_client::{HttpClient, HttpClientBuilder},
    ws_client::{WsClient, WsClientBuilder},
};
use sui_node::SuiNodeHandle;
use sui_sdk::{wallet_context::WalletContext, SuiClient, SuiClientBuilder};

use crate::{LocalClusterBuilder, LocalClusterConfig};

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
pub struct TestCluster {
    pub swarm: Swarm,
    pub wallet: WalletContext,
    pub fullnode_handle: FullNodeHandle,
}
impl TestCluster {}
pub struct LocalNewCluster {
    test_cluster: TestCluster,
}

impl LocalNewCluster {
    pub async fn start(config: &LocalClusterConfig) -> Result<Self, anyhow::Error> {
        let mut cluster_builder = LocalClusterBuilder::new();
        let mut test_cluster = cluster_builder.build().await;
        // Let nodes connect to one another
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // TODO: test connectivity before proceeding?
        Ok(Self { test_cluster })
    }
}
