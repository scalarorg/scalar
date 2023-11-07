use std::net::SocketAddr;

use scalar_tss::{
    encrypted_sled, gg20, kv_manager, mnemonic, service::Gg20Service,
    tss_peer_server::TssPeerServer, TssPeerService,
};
// mod multisig;

// gather logs; need to set RUST_LOG=info
use tracing::{info, span, Level};

// error handling
pub type TofndResult<Success> = anyhow::Result<Success>;

// protocol buffers via tonic: https://github.com/hyperium/tonic/blob/master/examples/helloworld-tutorial.md#writing-our-server
// pub mod proto {
//     tonic::include_proto!("tofnd");
// }

mod config;
use config::parse_args;

use crate::kv_manager::KvManager;

fn set_up_logs() {
    // enable only tofnd and tofn debug logs - disable serde, tonic, tokio, etc.
    tracing_subscriber::fmt()
        .with_env_filter("tofnd=debug,tofn=debug")
        .json()
        .with_ansi(atty::is(atty::Stream::Stdout))
        .with_target(false)
        .with_current_span(false)
        .flatten_event(true) // make logs complient with datadog
        .init();
}

#[cfg(feature = "malicious")]
pub fn warn_for_malicious_build() {
    use tracing::warn;
    warn!("WARNING: THIS tofnd BINARY AS COMPILED IN 'MALICIOUS' MODE.  MALICIOUS BEHAVIOUR IS INTENTIONALLY INSERTED INTO SOME MESSAGES.  THIS BEHAVIOUR WILL CAUSE OTHER tofnd PROCESSES TO IDENTIFY THE CURRENT PROCESS AS MALICIOUS.");
}

fn warn_for_unsafe_execution() {
    use tracing::warn;
    warn!("WARNING: THIS tofnd BINARY IS NOT SAFE: SAFE PRIMES ARE NOT USED BECAUSE '--unsafe' FLAG IS ENABLED.  USE '--unsafe' FLAG ONLY FOR TESTING.");
}

const SERVER_NAME: &str = "tofnd";

/// worker_threads defaults to the number of cpus on the system
/// https://docs.rs/tokio/1.2.0/tokio/attr.main.html#multi-threaded-runtime
#[tokio::main(flavor = "multi_thread")]
async fn main() -> TofndResult<()> {
    set_up_logs(); // can't print any logs until they're set up
    let cfg = parse_args()?;
    let socket_address = addr(&cfg.ip, cfg.port)?;

    // immediately read an encryption password from stdin
    let password = cfg.password_method.execute()?;

    // print config warnings
    #[cfg(feature = "malicious")]
    warn_for_malicious_build();
    if !cfg.safe_keygen {
        warn_for_unsafe_execution();
    }

    // set up span for logs
    let main_span = span!(Level::INFO, "main");
    let _enter = main_span.enter();
    let cmd = cfg.mnemonic_cmd.clone();

    // this step takes a long time due to password-based decryption
    let kv_manager = KvManager::new(cfg.tofnd_path.clone(), password)?
        .handle_mnemonic(&cfg.mnemonic_cmd)
        .await?;

    // let gg20_service = gg20::service::new_service(cfg, kv_manager.clone());
    // let multisig_service = multisig::service::new_service(kv_manager);

    if cmd.exit_after_cmd() {
        info!("Tofnd exited after using command <{:?}>. Run `./tofnd -m existing` to execute gRPC daemon.", cmd);
        return Ok(());
    }

    // let gg20_service = proto::gg20_server::Gg20Server::new(gg20_service);
    // let multisig_service = proto::multisig_server::MultisigServer::new(multisig_service);

    // let incoming = TcpListener::bind(socket_address).await?;
    // info!(
    //     "tofnd listen addr {:?}, use ctrl+c to shutdown",
    //     incoming.local_addr()?
    // );

    // tonic::transport::Server::builder()
    //     .add_service(gg20_service)
    //     .serve_with_incoming_shutdown(TcpListenerStream::new(incoming), shutdown_signal())
    //     .await?;

    let anemo_config = {
        let mut quic_config = anemo::QuicConfig::default();
        // Allow more concurrent streams for burst activity.
        quic_config.max_concurrent_bidi_streams = Some(10_000);
        // Increase send and receive buffer sizes on the primary, since the primary also
        // needs to fetch payloads.
        // With 200MiB buffer size and ~500ms RTT, the max throughput ~400MiB/s.
        quic_config.stream_receive_window = Some(100 << 20);
        quic_config.receive_window = Some(200 << 20);
        quic_config.send_window = Some(200 << 20);
        quic_config.crypto_buffer_size = Some(1 << 20);
        quic_config.socket_receive_buffer_size = Some(20 << 20);
        quic_config.socket_send_buffer_size = Some(20 << 20);
        quic_config.allow_failed_socket_buffer_size_setting = true;
        quic_config.max_idle_timeout_ms = Some(30_000);
        // Enable keep alives every 5s
        quic_config.keep_alive_interval_ms = Some(5_000);
        let mut config = anemo::Config::default();
        config.quic = Some(quic_config);
        // Set the max_frame_size to be 2 GB to work around the issue of there being too many
        // delegation events in the epoch change txn.
        config.max_frame_size = Some(2 << 30);
        // Set a default timeout of 300s for all RPC requests
        config.inbound_request_timeout_ms = Some(300_000);
        config.outbound_request_timeout_ms = Some(300_000);
        config.shutdown_idle_timeout_ms = Some(1_000);
        config.connectivity_check_interval_ms = Some(2_000);
        config.connection_backoff_ms = Some(1_000);
        config.max_connection_backoff_ms = Some(20_000);
        config
    };

    let (tx_keygen, mut rx_keygen) = tokio::sync::mpsc::unbounded_channel();
    let (tx_sign, mut rx_sign) = tokio::sync::mpsc::unbounded_channel();

    let tss_service = TssPeerServer::new(TssPeerService::new(tx_keygen, tx_sign));

    let network = build_network(
        |router| router.add_rpc_service(tss_service),
        socket_address,
        anemo_config,
    );

    Ok(())
}

fn addr(ip: &str, port: u16) -> TofndResult<SocketAddr> {
    let socket_addr = format!("{}:{}", ip, port);
    socket_addr
        .parse::<SocketAddr>()
        .map_err(|err| anyhow::anyhow!(err))
}

// graceful shutdown https://hyper.rs/guides/server/graceful-shutdown/
// can't use Result<> here because `serve_with_incoming_shutdown` expects F: Future<Output = ()>,
async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    info!("tofnd shutdown signal received");
}

pub fn random_key() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rng, &mut bytes[..]);
    bytes
}

pub fn build_network(
    f: impl FnOnce(anemo::Router) -> anemo::Router,
    address: SocketAddr,
    anemo_config: anemo::Config,
) -> anemo::Network {
    let router = f(anemo::Router::new());
    let network = anemo::Network::bind(address)
        .config(anemo_config)
        .private_key(random_key())
        .server_name(SERVER_NAME)
        .start(router)
        .unwrap();

    network
}

// #[cfg(test)]
// mod tests;
