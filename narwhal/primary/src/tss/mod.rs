mod multisig;
mod tss_keygen;
mod tss_party;
mod tss_service;
mod tss_signer;

use anemo::PeerId;
use crypto::NetworkPublicKey;

pub use multisig::*;
use network::CancelOnDropHandler;
use network::RetryConfig;
use std::{net::Ipv4Addr, sync::Arc};
use tonic::transport::Channel;
use tracing::{info, warn};
pub use tss_party::*;
pub use tss_service::*;
pub use tss_signer::*;
use types::multisig_client::MultisigClient;
use types::{
    gg20_client, message_in,
    message_out::{self, KeygenResult},
    KeygenOutput, MessageIn, TrafficIn,
};
use types::{gg20_client::Gg20Client, KeygenInit};
use types::{ConditionalBroadcastReceiver, SignInit};

pub async fn create_tofnd_client(
    port: u16,
) -> Result<Gg20Client<Channel>, tonic::transport::Error> {
    let tss_host = std::env::var("TSS_HOST").unwrap_or_else(|_| Ipv4Addr::LOCALHOST.to_string());
    let tss_port = std::env::var("TSS_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or_else(|| port);
    //+ authority.id().0;
    let tss_addr = format!("http://{}:{}", tss_host, tss_port);
    info!("TSS address {}", &tss_addr);
    let tofnd_client = Gg20Client::connect(tss_addr.clone()).await;
    tofnd_client
}

pub async fn create_multisig_client(
    port: u16,
) -> Result<MultisigClient<Channel>, tonic::transport::Error> {
    let tss_host = std::env::var("TSS_HOST").unwrap_or_else(|_| Ipv4Addr::LOCALHOST.to_string());
    let tss_port = std::env::var("TSS_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or_else(|| port);
    //+ authority.id().0;
    let tss_addr = format!("http://{}:{}", tss_host, tss_port);
    info!("TSS address {}", &tss_addr);
    MultisigClient::connect(tss_addr.clone()).await
}

pub fn send<F, R, Fut>(
    network: anemo::Network,
    peer: NetworkPublicKey,
    f: F,
) -> CancelOnDropHandler<anyhow::Result<anemo::Response<R>>>
where
    F: Fn(anemo::Peer) -> Fut + Send + Sync + 'static + Clone,
    R: Send + Sync + 'static + Clone,
    Fut: std::future::Future<Output = Result<anemo::Response<R>, anemo::rpc::Status>> + Send,
{
    // Safety
    // Since this spawns an unbounded task, this should be called in a time-restricted fashion.

    let peer_id = PeerId(peer.0.to_bytes());
    let message_send = move || {
        let network = network.clone();
        let f = f.clone();

        async move {
            if let Some(peer) = network.peer(peer_id) {
                f(peer).await.map_err(|e| {
                    // this returns a backoff::Error::Transient
                    // so that if anemo::Status is returned, we retry
                    backoff::Error::transient(anyhow::anyhow!("RPC error: {e:?}"))
                })
            } else {
                Err(backoff::Error::transient(anyhow::anyhow!(
                    "not connected to peer {peer_id}"
                )))
            }
        }
    };

    let retry_config = RetryConfig {
        retrying_max_elapsed_time: None, // retry forever
        ..Default::default()
    };
    let task = tokio::spawn(retry_config.retry(message_send));

    CancelOnDropHandler(task)
}
