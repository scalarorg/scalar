use std::net::SocketAddr;

use narwhal_config::{committee, Authority, Committee};
use narwhal_test_utils::CommitteeFixture;
use narwhal_types::PreSubscribedBroadcastSender;

use crate::{storage::TssStore, tss_peer_server::TssPeerServer, TssParty, TssPeerService};

mod keygen_tests;

const SERVER_NAME: &str = "tofnd";

// authority: Authority,
//         committee: Committee,
//         network: Network,
//         tss_store: TssStore,
//         tss_keygen: TssKeyGenerator,
//         tss_signer: TssSigner,
//         tx_keygen: UnboundedSender<MessageIn>,
//         tx_sign: UnboundedSender<MessageIn>,
//         tx_tss_sign_result: UnboundedSender<(SignInit, SignResult)>,

fn setup_party() {
    // Channel for communication between Tss spawn and Anemo Tss service
    let (tx_keygen, mut rx_keygen) = tokio::sync::mpsc::unbounded_channel();
    let (tx_sign, mut rx_sign) = tokio::sync::mpsc::unbounded_channel();
    let (tx_message_out, rx_message_out) = tokio::sync::mpsc::unbounded_channel();
    // Send sign init from Scalar Event handler to Tss spawn
    let (tx_tss_sign_init, rx_tss_sign_init) = tokio::sync::mpsc::unbounded_channel();
    // Send sign result from tss spawn to Scalar handler
    let (tx_tss_sign_result, rx_tss_sign_result) = tokio::sync::mpsc::unbounded_channel();

    // Shutdown channel
    let mut tx_shutdown = PreSubscribedBroadcastSender::new(100);

    let tss_service = TssPeerServer::new(TssPeerService::new(tx_keygen.clone(), tx_sign.clone()));

    let network = build_network(
        |router| router.add_rpc_service(tss_service),
        addr("0.0.0.0", 0),
        get_anemo_config(),
    );

    let fixture = CommitteeFixture::builder().build();

    let committee = fixture.committee();

    let author = fixture.authorities().last().unwrap();

    let tss_store = TssStore::new_for_tests();
    // let tss_handles = TssParty::spawn_v2(
    //             authority.clone(),
    //             committee.clone(),
    //             network.clone(),
    //             tss_store,
    //             tx_tss_keygen,
    //             rx_tss_keygen,
    //             tx_tss_sign,
    //             rx_tss_sign,
    //             rx_message_out,
    //             rx_tss_sign_init,
    //             tx_tss_sign_result,
    //             tx_shutdown.subscribe(),
    //         );
    let party = TssParty::spawn_v2(
        author.authority().clone(),
        committee,
        network,
        tss_store,
        tx_keygen,
        rx_keygen,
        tx_sign,
        rx_sign,
        rx_message_out,
        rx_tss_sign_init,
        tx_tss_sign_result,
        tx_shutdown.subscribe(),
    );
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

fn addr(ip: &str, port: u16) -> SocketAddr {
    let socket_addr = format!("{}:{}", ip, port);
    socket_addr.parse::<SocketAddr>().unwrap()
}

fn get_anemo_config() -> anemo::Config {
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

    anemo_config
}
