pub mod encrypted_sled;
pub mod gg20;
pub mod kv_manager;
pub mod mnemonic;
mod tests;
// mod multisig;
pub mod helper;
pub mod proto;
pub mod storage;
pub mod tss;
pub mod tss_keygen;
pub mod tss_party;
pub mod tss_service;
pub mod tss_signer;
pub mod types;

use anemo::PeerId;
use crypto::NetworkPublicKey;
pub use gg20::*;
// use narwhal_network::CancelOnDropHandler;
use narwhal_network::RetryConfig;
use std::net::Ipv4Addr;
use tonic::transport::Channel;
use tracing::info;
pub use tss_party::*;
pub use tss_service::*;
pub use tss_signer::*;
pub use types::gg20_client::Gg20Client;
pub use types::*;
pub type TofndResult<R> = anyhow::Result<R>;

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

/// This adapter will make a [`tokio::task::JoinHandle`] abort its handled task when the handle is dropped.
#[derive(Debug)]
#[must_use]
pub struct CancelOnDropHandler<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for CancelOnDropHandler<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl<T> std::future::Future for CancelOnDropHandler<T> {
    type Output = T;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use futures::future::FutureExt;
        // If the task panics just propagate it up
        self.0.poll_unpin(cx).map(Result::unwrap)
    }
}
