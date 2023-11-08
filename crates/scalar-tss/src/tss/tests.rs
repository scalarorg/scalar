use anemo::{Network, Request, Response};
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{convert::Infallible, time::Duration};
use tower::{util::BoxCloneService, ServiceExt};
use tracing::trace;

#[tokio::test]
async fn basic_network() -> Result<()> {
    let msg = b"The Way of Kings";

    let network_1 = build_network()?;
    let network_2 = build_network()?;

    let peer = network_1.connect(network_2.local_addr()).await?;
    let response = network_1
        .rpc(peer, Request::new(msg.as_ref().into()))
        .await?;
    assert_eq!(response.into_body(), msg.as_ref());

    let msg = b"Words of Radiance";
    let peer_id_1 = network_1.peer_id();
    let response = network_2
        .rpc(peer_id_1, Request::new(msg.as_ref().into()))
        .await?;
    assert_eq!(response.into_body(), msg.as_ref());
    Ok(())
}

fn build_network() -> Result<Network> {
    build_network_with_addr("localhost:0")
}

fn build_network_with_addr(addr: &str) -> Result<Network> {
    let network = Network::bind(addr)
        .private_key(random_private_key())
        .server_name("test")
        .start(echo_service())?;

    trace!(
        address =% network.local_addr(),
        peer_id =% network.peer_id(),
        "starting network"
    );

    Ok(network)
}

fn random_private_key() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rng, &mut bytes[..]);

    bytes
}

fn echo_service() -> BoxCloneService<Request<Bytes>, Response<Bytes>, Infallible> {
    let handle = move |request: Request<Bytes>| async move {
        trace!("received: {}", request.body().escape_ascii());
        let response = Response::new(request.into_body());
        Result::<Response<Bytes>, Infallible>::Ok(response)
    };

    tower::service_fn(handle).boxed_clone()
}
