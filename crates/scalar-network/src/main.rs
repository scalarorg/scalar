use clap::Parser;
use futures::future::join_all;
use scalar_network::broadcaster::Broadcaster;
use scalar_network::chat::chat_server::ChatServer;
use scalar_network::chat::ChatPeerServer;
use scalar_network::cli::ChatCli;
use scalar_network::config::Args;
use scalar_network::discovery::builder::DiscoveryBuilder;
use scalar_network::reconnect::ChatReconnect;
use scalar_network::utils::build_network;
use tokio::join;
use tokio::sync::mpsc;
use tracing::Level;

fn set_up_logs() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

#[tokio::main]
async fn main() {
    let config = Args::parse();
    let known_peer_config = config.get_known_peer_config();
    let network_config = config.get_network_config();

    set_up_logs();
    let mut handles = vec![];

    let (tx, rx) = mpsc::channel(config.max_chat_channels);
    let (discovery_builder, discovery_server, state) =
        DiscoveryBuilder::new().config(known_peer_config).build();

    let network = build_network(
        |router| {
            router
                .add_rpc_service(ChatPeerServer::new(ChatServer::default()))
                .add_rpc_service(discovery_server)
        },
        network_config,
        config.get_anemo_config(),
    );

    // Start the discovery event loop
    {
        let discovery_handle = discovery_builder.start(network.clone());
        handles.push(discovery_handle);
    }

    // Start the command loop
    {
        let mut command_loop = Broadcaster::new(state, network, known_peer_config, rx);
        let commmand_handle = tokio::spawn(async move {
            command_loop.start().await;
        });
        handles.push(commmand_handle);
    }

    // Reconnect every x milliseconds (30,000ms default)
    {
        let reconnect_tx = tx.clone();
        let reconnect_handle =
            ChatReconnect::new(reconnect_tx, config.network_reconnect_interval).start();
        handles.push(reconnect_handle);
    }

    // Start the CLI
    let cli = ChatCli::new(tx);
    let cli_future = cli.start();

    join!(join_all(handles), cli_future);
}
