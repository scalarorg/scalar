//! Scalar reth extend
use clap::Args;
use reth_basic_payload_builder::{BasicPayloadJobGenerator, BasicPayloadJobGeneratorConfig};
use reth_payload_builder::{PayloadBuilderHandle, PayloadBuilderService};
use reth_tasks::TaskSpawner;

use crate::cli::{
    components::{RethNodeComponents, RethRpcComponents, RethRpcServerHandles},
    config::{PayloadBuilderConfig, RethNetworkConfig, RethRpcConfig},
    ext::{RethCliExt, RethNodeCommandConfig},
};

/// The Scalar Ext configuration for the reth node command
/// [Command](crate::commands::node::NodeCommand).
///
/// This is a convenience type for [NoArgs<()>].
#[derive(Debug, Clone, Copy, Default, Args)]
#[non_exhaustive]
pub struct ScalarRethNodeCommandConfig;

impl RethNodeCommandConfig for ScalarRethNodeCommandConfig {
    /// Invoked with the network configuration before the network is configured.
    ///
    /// This allows additional configuration of the network before it is launched.
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
        let _ = components;
        Ok(())
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
