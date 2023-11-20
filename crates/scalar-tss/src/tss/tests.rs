use std::path::Path;

use super::{
    builder::{PartyBuilder, UnstartedParty},
    party::TssParty,
};
use crate::{
    message_out::{self, KeygenResult, SignResult},
    storage::TssStore,
    KeygenOutput, SignInit,
};
use anemo::{types::Address, Config, PeerId};
use narwhal_config::{Authority, Committee};
use narwhal_types::PreSubscribedBroadcastSender;
use tokio::{sync::mpsc::UnboundedSender, task::JoinHandle};
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;
use tracing::info;

#[tokio::test]
async fn test_basic_case() -> anyhow::Result<()> {
    basic_keygen_and_sign(false).await
}

#[tokio::test]
async fn test_basic_case_with_recovery() -> anyhow::Result<()> {
    basic_keygen_and_sign(true).await
}

const PARTY_COUNT: usize = 4;

#[cfg(test)]
pub async fn basic_keygen_and_sign(recover: bool) -> anyhow::Result<()> {
    const NUM_SHUTDOWN_RECEIVERS: u64 = 10;
    use futures_util::future::join_all;
    use narwhal_test_utils::CommitteeFixture;
    use std::num::NonZeroUsize;
    use testdir::testdir;

    let dir = testdir!();

    let _guard = init_tracing_for_testing();

    let mut tx_shutdown = PreSubscribedBroadcastSender::new(NUM_SHUTDOWN_RECEIVERS);
    let mut all_handles = vec![];

    let fixture = CommitteeFixture::builder()
        .committee_size(NonZeroUsize::new(PARTY_COUNT).unwrap())
        .randomize_ports(true)
        .build();
    let committee = fixture.committee();

    // Setup all parties.
    let (parties, network) = setup_parties(committee.clone(), &dir).await;
    let channels = gather_party_channels(&parties);

    // Spawn all parties
    let (handles, parties) = spawn_parties(parties, network, &mut tx_shutdown, None).await;
    all_handles.extend(handles);

    // When the keygen result is received, send the sign init.
    let handle = tokio::spawn(async move {
        let (keygen_results, mut sign_init_msg) =
            send_keygen_and_sign_message(channels, committee.clone(), recover).await;

        match recover {
            true => {
                info!("Recover keygen results!");
                let keygen_output = gather_recover_info(&keygen_results);

                // Shutdown all parties.
                shutdown_parties(parties).await;

                // Reinitialize parties with recovered keygen results.
                let (parties, network) = setup_parties(committee.clone(), &dir).await;
                let mut channels = gather_party_channels(&parties);

                let (handles, _parties) =
                    spawn_parties(parties, network, &mut tx_shutdown, Some(keygen_output)).await;
                // all_handles.extend(handles);

                // Try sign again.
                sign_init_msg.key_uid = format!("tss_session{}", committee.epoch());
                for PartyChannel { tx_sign_init, .. } in &channels {
                    tx_sign_init
                        .send(sign_init_msg.clone())
                        .expect("Sign init should be sent successfully");
                }

                // Check if all default rx_sign_result are taken (None).
                let mut ready = 0;
                for PartyChannel { rx_sign_result, .. } in &mut channels {
                    if rx_sign_result
                        .next()
                        .await
                        .expect("Get initial data")
                        .is_none()
                    {
                        ready += 1;
                    }
                }
                assert_eq!(ready, PARTY_COUNT);

                // Wait for all rx_sign_result receive all sign results.
                for PartyChannel { rx_sign_result, .. } in &mut channels {
                    let sign_result = rx_sign_result.next().await.expect("Receive sign result");
                    assert!(sign_result
                        .expect("Sign result should be some")
                        .sign_result_data
                        .is_some());
                }

                info!("Receive all re-sign result!");

                tx_shutdown
                    .send()
                    .expect("Shutdown should be sent successfully");

                join_all(handles).await;
            }

            false => {
                // Send shutdown to all parties.
                tx_shutdown
                    .send()
                    .expect("Shutdown should be sent successfully");
            }
        }
    });

    all_handles.push(handle);

    join_all(all_handles).await;

    Ok(())
}

pub async fn shutdown_parties(mut parties: Vec<TssParty>) {
    for party in parties.iter_mut() {
        party.shutdown().await.expect("Shutdown successfully");
    }
    info!("Shutdown all parties!");
}

pub async fn send_keygen_and_sign_message(
    mut channels: Vec<PartyChannel>,
    committee: Committee,
    recover: bool,
) -> (Vec<KeygenResult>, SignInit) {
    // Check if all default rx_keygen_result are taken (None).
    let mut ready = 0;
    for PartyChannel {
        rx_keygen_result, ..
    } in &mut channels
    {
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

    let mut keygen_results = vec![];
    // Wait for all rx_keygen_result receive all keygen results.
    for PartyChannel {
        rx_keygen_result, ..
    } in &mut channels
    {
        let keygen_result = rx_keygen_result
            .next()
            .await
            .expect("Receive keygen result");

        assert!(keygen_result.is_some());

        keygen_results.push(keygen_result.unwrap());
    }

    info!("Receive all keygen result!");

    let party_uids = committee
        .authorities()
        .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
        .collect::<Vec<String>>()
        .clone();
    // Send a 32 byte message to the sign init channel.
    let message_to_sign = [1u8; 32].to_vec();

    info!("Message to sign has length {}", message_to_sign.len());

    let new_sig_uid = uuid::Uuid::new_v4().to_string();

    let key_uid = match recover {
        true => format!("tss_session{}", committee.epoch() + 1),
        false => format!("tss_session{}", committee.epoch()),
    };

    let sign_init_msg = SignInit {
        new_sig_uid: new_sig_uid.clone(),
        key_uid,
        party_uids: party_uids.clone(),
        message_to_sign: message_to_sign.clone(),
    };

    // Send sign init to all parties.
    for PartyChannel { tx_sign_init, .. } in &channels {
        tx_sign_init
            .send(sign_init_msg.clone())
            .expect("Sign init should be sent successfully");
    }

    // Check if all default rx_sign_result are taken (None).
    let mut ready = 0;
    for PartyChannel { rx_sign_result, .. } in &mut channels {
        if rx_sign_result
            .next()
            .await
            .expect("Get initial data")
            .is_none()
        {
            ready += 1;
        }
    }
    assert_eq!(ready, PARTY_COUNT);

    // Wait for all rx_sign_result receive all sign results.
    for PartyChannel { rx_sign_result, .. } in &mut channels {
        let sign_result = rx_sign_result
            .next()
            .await
            .expect("Receive sign result")
            .expect("Sign result should be some");

        match recover {
            true => {
                assert!(sign_result.sign_result_data.is_none());
            }
            false => {
                assert!(sign_result.sign_result_data.is_some());
            }
        }
    }

    info!("Receive all sign result!");

    (keygen_results, sign_init_msg)
}

pub async fn spawn_parties(
    parties: Vec<UnstartedParty>,
    networks: Vec<anemo::Network>,
    tx_shutdown: &mut PreSubscribedBroadcastSender,
    // If keygen_results is provided, then the keygens of the parties will be recovered.
    keygen_results: Option<Vec<KeygenOutput>>,
) -> (Vec<JoinHandle<()>>, Vec<TssParty>) {
    let mut all_handles = vec![];
    let mut spawned_parties = vec![];

    match keygen_results {
        Some(keygen_results) => {
            for ((builder, network), keygen_result) in parties
                .into_iter()
                .zip(networks.into_iter())
                .zip(keygen_results.into_iter())
            {
                let party = builder.build(network).await;
                party.execute_recover(keygen_result).await;

                let (handle, party) = builder.start_with_party(party, tx_shutdown.subscribe());
                all_handles.push(handle);
                spawned_parties.push(party);
            }
        }
        None => {
            for (party, network) in parties.into_iter().zip(networks.into_iter()) {
                let (handle, party) = party.start(network, tx_shutdown.subscribe()).await;
                all_handles.push(handle);
                spawned_parties.push(party);
            }
        }
    }

    (all_handles, spawned_parties)
}

pub fn gather_recover_info(results: &[KeygenResult]) -> Vec<KeygenOutput> {
    // gather recover info
    let mut recover_infos = vec![];
    for result in results.iter() {
        let result_data = result.keygen_result_data.clone().unwrap();
        match result_data {
            message_out::keygen_result::KeygenResultData::Data(output) => {
                recover_infos.push(output);
            }
            message_out::keygen_result::KeygenResultData::Criminals(_) => {}
        }
    }
    recover_infos
}

pub struct PartyChannel {
    pub rx_keygen_result: WatchStream<Option<KeygenResult>>,
    pub tx_sign_init: UnboundedSender<SignInit>,
    pub rx_sign_result: WatchStream<Option<SignResult>>,
}

pub fn gather_party_channels(parties: &[UnstartedParty]) -> Vec<PartyChannel> {
    let mut channels = vec![];

    for party in parties {
        let config = party.get_config();
        let rx_keygen_result = WatchStream::new(config.rx_keygen_result.clone());
        let tx_sign_init = config.tx_sign_init.clone();
        let rx_sign_result = WatchStream::new(config.rx_sign_result.clone());

        channels.push(PartyChannel {
            rx_keygen_result,
            tx_sign_init,
            rx_sign_result,
        });
    }

    channels
}

pub async fn setup_parties(
    committee: Committee,
    dir: &Path,
) -> (Vec<UnstartedParty>, Vec<anemo::Network>) {
    let (parties, networks) = init_parties(committee.clone(), dir);

    // Connect all party's networks to each other.
    connect_networks(&networks).await;

    (parties, networks)
}

pub fn init_parties(
    committee: Committee,
    dir: &Path,
) -> (Vec<UnstartedParty>, Vec<anemo::Network>) {
    let mut parties = vec![];
    let mut networks = vec![];
    for (index, authority) in committee.authorities().enumerate() {
        let (party, network) = init_party(committee.clone(), authority.clone(), dir, index);

        parties.push(party);
        networks.push(network);
    }

    (parties, networks)
}

pub async fn connect_networks(networks: &[anemo::Network]) {
    let mut connected = 0;

    // Connect all party's networks to each other.
    for network in networks {
        for other_network in networks {
            if network.local_addr().port() != other_network.local_addr().port() {
                network
                    .connect_with_peer_id(other_network.local_addr(), other_network.peer_id())
                    .await
                    .expect("Connect successfully");
                connected += 1;
            }
        }
    }

    // Make sure all parties are connected.
    assert_eq!(connected, networks.len() * (networks.len() - 1));
    for network in networks {
        assert_eq!(network.peers().len(), networks.len() - 1);
    }
}

pub fn init_party(
    committee: Committee,
    authority: Authority,
    testdir: &Path,
    party_index: usize,
) -> (UnstartedParty, anemo::Network) {
    let tofnd_path = format!("test-key-{:02}", party_index);
    let tofnd_path = testdir.join(tofnd_path);
    let address = authority.primary_address();
    let addr = address.to_anemo_address().unwrap();

    let tss_store = TssStore::new_for_tests();

    let (party, tss_server) = PartyBuilder::new(authority, committee, tss_store).build(tofnd_path);

    let network = build_network(addr, |router| router.add_rpc_service(tss_server));

    (party, network)
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
