use super::builder::{InternalPartyBuilder, PartyBuilder};
use crate::{storage::TssStore, SignInit};
use anemo::{types::Address, Config, PeerId};
use anyhow::Result;
use narwhal_config::{Authority, Committee};
use tracing::info;

#[tokio::test]
async fn emulate_keygen_and_sign_for_four_parties() -> Result<()> {
    use futures_util::future::join_all;
    use narwhal_test_utils::CommitteeFixture;
    use narwhal_types::PreSubscribedBroadcastSender;
    use tokio_stream::{wrappers::WatchStream, StreamExt};

    const PARTY_COUNT: usize = 4;
    const NUM_SHUTDOWN_RECEIVERS: u64 = 10;

    let _guard = init_tracing_for_testing();

    let mut tx_shutdown = PreSubscribedBroadcastSender::new(NUM_SHUTDOWN_RECEIVERS);
    let mut all_handles = vec![];

    let fixture = CommitteeFixture::builder().randomize_ports(true).build();
    let committee = fixture.committee();
    let mut authorities = fixture.authorities();

    // Spawn parties and collect their channels.
    let mut tss_channels = Vec::new();
    let mut parties = Vec::new();
    for _ in 0..PARTY_COUNT {
        let authority = authorities.next().unwrap().authority().clone();
        let party = init_party(committee.clone(), authority);
        let rx_keygen_result = WatchStream::new(party.0.get_config().rx_keygen_result.clone());
        let tx_sign_init = party.0.get_config().tx_sign_init.clone();
        tss_channels.push((rx_keygen_result, tx_sign_init));
        parties.push(party);
    }

    let mut connected = 0;

    // Connect all parties to each other.
    for (_, network) in &parties {
        for (_, other_network) in &parties {
            if network.local_addr().port() != other_network.local_addr().port() {
                network
                    .connect_with_peer_id(other_network.local_addr(), other_network.peer_id())
                    .await?;
                connected += 1;
            }
        }
    }

    // Make sure all parties are connected.
    assert_eq!(connected, PARTY_COUNT * (PARTY_COUNT - 1));
    for (_, network) in &parties {
        assert_eq!(network.peers().len(), PARTY_COUNT - 1);
    }

    // Spawn all parties
    for (builder, network) in parties {
        let rx_shutdown = tx_shutdown.subscribe();
        let handle = tokio::spawn(async move { builder.start(network, rx_shutdown) });
        all_handles.push(handle);
    }

    // When the keygen result is received, send the sign init.
    let handle = tokio::spawn(async move {
        // Check if all rx_keygen_result are ready.
        let mut ready = 0;
        for (rx_keygen_result, _) in &mut tss_channels {
            if rx_keygen_result
                .next()
                .await
                .expect("Get initial data")
                .is_none()
            {
                ready += 1;
            }
        }
        assert_eq!(ready, PARTY_COUNT);

        // Wait for all rx_keygen_result receive keygen result.
        for (rx_keygen_result, _) in &mut tss_channels {
            assert_eq!(
                rx_keygen_result
                    .next()
                    .await
                    .expect("Receive keygen result"),
                Some(())
            );
        }

        info!("Receive all keygen result!");

        let party_uids = committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>()
            .clone();
        // Send a 32 byte message to the sign init channel.
        let message_to_sign = [0u8; 32].to_vec();

        info!("Message to sign has length {}", message_to_sign.len());

        // Send sign init to all parties.
        for (_, tx_sign_init) in &tss_channels {
            tx_sign_init
                .send(SignInit {
                    new_sig_uid: uuid::Uuid::new_v4().to_string(),
                    key_uid: "tss_session".into(),
                    party_uids: party_uids.clone(),
                    message_to_sign: message_to_sign.clone(),
                })
                .expect("Sign init should be sent successfully");
        }

        // Sleep for 1 second to make sure all sign init messages are sent.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    });

    let handles = join_all(all_handles).await;

    // Get the result of all the handles.
    let handles = handles.into_iter().collect::<Result<Vec<_>, _>>()?;

    // Check that all handles completed successfully.
    for handle in handles {
        join_all(handle).await;
    }

    handle.await?;

    // TODO: Check that the keygen and sign results are correct.
    Ok(())
}

fn init_party(
    committee: Committee,
    authority: Authority,
) -> (InternalPartyBuilder, anemo::Network) {
    let address = authority.primary_address();
    let addr = address.to_anemo_address().unwrap();

    let tss_store = TssStore::new_for_tests();

    let (builder, tss_server, _) = PartyBuilder::new(authority, committee, tss_store).build();

    let network = build_network(addr, |router| router.add_rpc_service(tss_server));

    (builder, network)
}

pub fn random_key() -> [u8; 32] {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rng, &mut bytes[..]);
    bytes
}

pub fn build_network(
    address: Address,
    f: impl FnOnce(anemo::Router) -> anemo::Router,
) -> anemo::Network {
    let anemo_config: Config = {
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
    let router = f(anemo::Router::new());
    let network = anemo::Network::bind(address)
        .config(anemo_config)
        .private_key(random_key())
        .server_name("test")
        .start(router)
        .unwrap();

    info!("My local ip: {}", network.local_addr().ip());
    info!("My local port: {}", network.local_addr().port());
    network
}

#[cfg(test)]
fn init_tracing_for_testing() -> ::tracing::dispatcher::DefaultGuard {
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug,quinn=warn"));

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        // .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_test_writer()
        .finish();

    ::tracing::subscriber::set_default(subscriber)
}
