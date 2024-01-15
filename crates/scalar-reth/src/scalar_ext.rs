//! Scalar reth extend
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use backon::{ExponentialBuilder, Retryable};
use clap::Args;
use reth_basic_payload_builder::{BasicPayloadJobGenerator, BasicPayloadJobGeneratorConfig};
use reth_beacon_consensus::BeaconEngineMessage;
use reth_payload_builder::{PayloadBuilderHandle, PayloadBuilderService};
use reth_provider::providers::BlockchainProvider;
use reth_tasks::TaskSpawner;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::{
    consensus_adapter::{
        beacon_client::BeaconConsensusClient,
        builder::{start_consensus_client, AdapterBuilder},
    },
    proto::ExternalTransaction,
};
use reth::cli::{
    components::{RethNodeComponents, RethRpcComponents, RethRpcServerHandles},
    config::{PayloadBuilderConfig, RethNetworkConfig, RethRpcConfig},
    ext::{RethCliExt, RethNodeCommandConfig, RethNodeCommandExt},
};
use tracing::{error, info};

/// default grpc consensus component's port
pub const DEFAULT_CONSENSUS_PROTOCOL: &str = "narwhal";
/// default grpc consensus component's port
pub const DEFAULT_CONSENSUS_PORT: u16 = 8555;
/// default block time
pub const DEFAULT_BLOCK_TIME: u32 = 12000;
/// Maximum number of transactions in each block
pub const DEFAULT_MAX_BLOCK_TRANSACTIONS: u32 = 2000;

#[derive(Debug, Clone, Args)]
pub struct ConsensusArgs {
    /// Enable the Narwhal client
    #[arg(long = "consensus.enable", default_value_if("dev", "true", "true"))]
    pub consensus_enable: bool,

    /// Http server address to listen on
    #[arg(long = "consensus.protocol", default_value = DEFAULT_CONSENSUS_PROTOCOL)]
    pub consensus_protocol: String,

    /// Http server address to listen on
    #[arg(long = "consensus.addr", default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    pub consensus_addr: IpAddr,

    /// Http server port to listen on
    #[arg(long = "consensus.port", default_value_t = DEFAULT_CONSENSUS_PORT)]
    pub consensus_port: u16,

    /// Time out for seal a new block with lackof transactions
    #[arg(long = "consensus.blocktime", default_value_t = DEFAULT_BLOCK_TIME)]
    pub consensus_blocktime: u32,

    /// Maximum number of transactions included in a block
    #[arg(long = "consensus.max_transactions", default_value_t = DEFAULT_MAX_BLOCK_TRANSACTIONS)]
    pub consensus_max_transactions: u32,
}

impl Default for ConsensusArgs {
    fn default() -> Self {
        Self {
            consensus_enable: false,
            consensus_protocol: String::from(DEFAULT_CONSENSUS_PROTOCOL),
            consensus_addr: Ipv4Addr::LOCALHOST.into(),
            consensus_port: DEFAULT_CONSENSUS_PORT,
            consensus_blocktime: DEFAULT_BLOCK_TIME,
            consensus_max_transactions: DEFAULT_MAX_BLOCK_TRANSACTIONS,
        }
    }
}

/// The Scalar Ext configuration for the reth node command
/// [Command](crate::commands::node::NodeCommand).
///
/// This is a convenience type for [NoArgs<()>].
#[derive(Debug, Clone, Default, Args)]
#[non_exhaustive]
pub struct ScalarRethNodeCommandConfig {
    #[clap(flatten)]
    consensus: Option<ConsensusArgs>,
}

impl ScalarRethNodeCommandConfig {
    fn start_consensus_adapter<Reth: RethNodeComponents>(
        &mut self,
        components: &Reth,
    ) -> eyre::Result<()> {
        info!(target: "scalar::cli", "Starting Reth with consensus config {:?}", &self.consensus);
        let ConsensusArgs {
            consensus_addr,
            consensus_port,
            ..
        } = self.consensus.clone().unwrap_or_default();
        let pool = components.pool();
        let (tx_committed_transactions, rx_committed_transactions) =
            unbounded_channel::<Vec<ExternalTransaction>>();
        let (tx_engine, rx_engine) = unbounded_channel::<BeaconEngineMessage>();
        let mining_task = AdapterBuilder::new(
            Arc::clone(&components.chain_spec()),
            components.provider(),
            pool.clone(),
            tx_engine,
            rx_committed_transactions,
        )
        .build();
        let task_executor = components.task_executor();
        task_executor.spawn(Box::pin(mining_task));
        //Start tokio task for send request to the consensus engine api
        let beacon_client = BeaconConsensusClient::new(rx_engine);
        let _beacon_client_handle = tokio::spawn(async {
            beacon_client.start().await;
        });
        let consensus_address = SocketAddr::new(consensus_addr, consensus_port);
        tokio::spawn(async move {
            (move || {
                start_consensus_client(
                    consensus_address.clone(),
                    pool.clone(),
                    tx_committed_transactions.clone(),
                )
            })
            .retry(&ExponentialBuilder::default().with_max_times(100_000))
            .notify(|err, _| error!("Scalar consensus client error: {}. Retrying...", err))
            .await
        });
        // let (_, client, mut task) = ScalarBuilder::new(
        //     Arc::clone(&self.config.chain),
        //     blockchain_db.clone(),
        //     transaction_pool.clone(),
        //     mining_mode,
        //     consensus_engine_tx.clone(),
        //     canon_state_notification_sender,
        //     tx_commited_transactions,
        //     self.config.consensus.clone(),
        // )
        // .build();

        // let mut pipeline = self
        //     .config
        //     .build_networked_pipeline(
        //         &config.stages,
        //         client.clone(),
        //         Arc::clone(&consensus),
        //         provider_factory.clone(),
        //         &executor.clone(),
        //         sync_metrics_tx,
        //         prune_config.clone(),
        //         max_block,
        //     )
        //     .await?;

        // let pipeline_events = pipeline.events();
        // task.set_pipeline_events(pipeline_events);
        // debug!(target: "reth::cli", "Spawning auto mine task");
        // executor.spawn(Box::pin(task));
        // (pipeline, EitherDownloader::Scalar(client))
        Ok(())
    }
}

impl RethNodeCommandConfig for ScalarRethNodeCommandConfig {
    /// Invoked with the network configuration before the network is configured.
    ///
    /// This allows additional configuration of the network before it is launched.
    /// Called in Node command
    fn configure_network<Conf, Reth>(
        &mut self,
        config: &mut Conf,
        components: &Reth,
    ) -> eyre::Result<()>
    where
        Conf: RethNetworkConfig,
        Reth: RethNodeComponents,
    {
        let _ = config;
        let _ = components;
        Ok(())
    }

    /// Event hook called once all components have been initialized.
    ///
    /// This is called as soon as the node components have been initialized.
    fn on_components_initialized<Reth: RethNodeComponents>(
        &mut self,
        components: &Reth,
    ) -> eyre::Result<()> {
        let _ = components;
        Ok(())
    }

    /// Event hook called once the node has been launched.
    ///
    /// This is called last after the node has been launched.
    fn on_node_started<Reth: RethNodeComponents>(&mut self, components: &Reth) -> eyre::Result<()> {
        self.start_consensus_adapter(components)
    }

    /// Event hook called once the rpc servers has been started.
    ///
    /// This is called after the rpc server has been started.
    fn on_rpc_server_started<Conf, Reth>(
        &mut self,
        config: &Conf,
        components: &Reth,
        rpc_components: RethRpcComponents<'_, Reth>,
        handles: RethRpcServerHandles,
    ) -> eyre::Result<()>
    where
        Conf: RethRpcConfig,
        Reth: RethNodeComponents,
    {
        let _ = config;
        let _ = components;
        let _ = rpc_components;
        let _ = handles;
        Ok(())
    }

    /// Allows for registering additional RPC modules for the transports.
    ///
    /// This is expected to call the merge functions of [reth_rpc_builder::TransportRpcModules], for
    /// example [reth_rpc_builder::TransportRpcModules::merge_configured].
    ///
    /// This is called before the rpc server will be started [Self::on_rpc_server_started].
    fn extend_rpc_modules<Conf, Reth>(
        &mut self,
        config: &Conf,
        components: &Reth,
        rpc_components: RethRpcComponents<'_, Reth>,
    ) -> eyre::Result<()>
    where
        Conf: RethRpcConfig,
        Reth: RethNodeComponents,
    {
        let _ = config;
        let _ = components;
        let _ = rpc_components;
        Ok(())
    }

    /// Configures the [PayloadBuilderService] for the node, spawns it and returns the
    /// [PayloadBuilderHandle].
    ///
    /// By default this spawns a [BasicPayloadJobGenerator] with the default configuration
    /// [BasicPayloadJobGeneratorConfig].
    fn spawn_payload_builder_service<Conf, Reth>(
        &mut self,
        conf: &Conf,
        components: &Reth,
    ) -> eyre::Result<PayloadBuilderHandle>
    where
        Conf: PayloadBuilderConfig,
        Reth: RethNodeComponents,
    {
        let payload_job_config = BasicPayloadJobGeneratorConfig::default()
            .interval(conf.interval())
            .deadline(conf.deadline())
            .max_payload_tasks(conf.max_payload_tasks())
            .extradata(conf.extradata_rlp_bytes())
            .max_gas_limit(conf.max_gas_limit());

        // no extradata for optimism
        #[cfg(feature = "optimism")]
        let payload_job_config = payload_job_config.extradata(Default::default());

        // The default payload builder is implemented on the unit type.
        #[cfg(not(feature = "optimism"))]
        let payload_builder = reth_ethereum_payload_builder::EthereumPayloadBuilder::default();

        // Optimism's payload builder is implemented on the OptimismPayloadBuilder type.
        #[cfg(feature = "optimism")]
        let payload_builder = reth_optimism_payload_builder::OptimismPayloadBuilder::default()
            .set_compute_pending_block(conf.compute_pending_block());

        let payload_generator = BasicPayloadJobGenerator::with_builder(
            components.provider(),
            components.pool(),
            components.task_executor(),
            payload_job_config,
            components.chain_spec(),
            payload_builder,
        );
        let (payload_service, payload_builder) = PayloadBuilderService::new(payload_generator);

        components
            .task_executor()
            .spawn_critical("payload builder service", Box::pin(payload_service));

        Ok(payload_builder)
    }
}

/// Extend Cli for Reth
pub struct ScalarExt {}

impl RethCliExt for ScalarExt {
    type Node = ScalarRethNodeCommandConfig;
}
