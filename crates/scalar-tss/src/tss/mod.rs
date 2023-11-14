use anemo::PeerId;
use narwhal_network::RetryConfig;
pub mod builder;
pub mod keygen;
pub mod party;
pub mod service;
pub mod signer;
pub mod tests;

pub fn send<F, R, Fut>(
    network: anemo::Network,
    peer_id: PeerId,
    f: F,
) -> CancelOnDropHandler<anyhow::Result<anemo::Response<R>>>
where
    F: Fn(anemo::Peer) -> Fut + Send + Sync + 'static + Clone,
    R: Send + Sync + 'static + Clone,
    Fut: std::future::Future<Output = Result<anemo::Response<R>, anemo::rpc::Status>> + Send,
{
    // Safety
    // Since this spawns an unbounded task, this should be called in a time-restricted fashion.

    // let peer_id = PeerId(peer.0.to_bytes());
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
