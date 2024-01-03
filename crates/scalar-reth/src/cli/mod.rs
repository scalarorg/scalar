//! CLI definition and entrypoint to executable

use crate::{
    args::{
        utils::{chain_help, genesis_value_parser, SUPPORTED_CHAINS},
        LogArgs,
    },
    cli::ext::RethCliExt,
    commands::{
        config_cmd, db, debug_cmd, import, init_cmd, node, p2p, recover, stage, test_vectors,
    },
    runner::CliRunner,
    version::{LONG_VERSION, SHORT_VERSION},
};
use clap::{value_parser, ArgAction, Args, Parser, Subcommand};
use reth_primitives::ChainSpec;
use reth_tracing::{
    tracing::{metadata::LevelFilter, Level},
    tracing_subscriber::filter::Directive,
    FileWorkerGuard,
};
use std::sync::Arc;

pub mod components;
pub mod config;
pub mod db_type;
pub mod ext;

/// The main reth cli interface.
///
/// This is the entrypoint to the executable.
#[derive(Debug, Parser)]
#[command(author, version = SHORT_VERSION, long_version = LONG_VERSION, about = "Reth", long_about = None)]
pub struct Cli<Ext: RethCliExt = ()> {
    /// The command to run
    #[clap(subcommand)]
    command: Commands<Ext>,

    /// The chain this node is running.
    ///
    /// Possible values are either a built-in chain or the path to a chain specification file.
    #[arg(
        long,
        value_name = "CHAIN_OR_PATH",
        long_help = chain_help(),
        default_value = SUPPORTED_CHAINS[0],
        value_parser = genesis_value_parser,
        global = true,
    )]
    chain: Arc<ChainSpec>,

    /// Add a new instance of a node.
    ///
    /// Configures the ports of the node to avoid conflicts with the defaults.
    /// This is useful for running multiple nodes on the same machine.
    ///
    /// Max number of instances is 200. It is chosen in a way so that it's not possible to have
    /// port numbers that conflict with each other.
    ///
    /// Changes to the following port numbers:
    /// - DISCOVERY_PORT: default + `instance` - 1
    /// - AUTH_PORT: default + `instance` * 100 - 100
    /// - HTTP_RPC_PORT: default - `instance` + 1
    /// - WS_RPC_PORT: default + `instance` * 2 - 2
    #[arg(long, value_name = "INSTANCE", global = true, default_value_t = 1, value_parser = value_parser!(u16).range(..=200))]
    instance: u16,

    #[clap(flatten)]
    logs: LogArgs,

    #[clap(flatten)]
    verbosity: Verbosity,
}

impl<Ext: RethCliExt> Cli<Ext> {
    /// Execute the configured cli command.
    pub fn run(mut self) -> eyre::Result<()> {
        // add network name to logs dir
        self.logs.log_file_directory = self
            .logs
            .log_file_directory
            .join(self.chain.chain.to_string());

        let _guard = self.init_tracing()?;

        let runner = CliRunner;
        match self.command {
            Commands::Node(command) => runner.run_command_until_exit(|ctx| command.execute(ctx)),
            Commands::Init(command) => runner.run_blocking_until_ctrl_c(command.execute()),
            Commands::Import(command) => runner.run_blocking_until_ctrl_c(command.execute()),
            Commands::Db(command) => runner.run_blocking_until_ctrl_c(command.execute()),
            Commands::Stage(command) => runner.run_blocking_until_ctrl_c(command.execute()),
            Commands::P2P(command) => runner.run_until_ctrl_c(command.execute()),
            Commands::TestVectors(command) => runner.run_until_ctrl_c(command.execute()),
            Commands::Config(command) => runner.run_until_ctrl_c(command.execute()),
            Commands::Debug(command) => runner.run_command_until_exit(|ctx| command.execute(ctx)),
            Commands::Recover(command) => runner.run_command_until_exit(|ctx| command.execute(ctx)),
        }
    }

    /// Initializes tracing with the configured options.
    ///
    /// If file logging is enabled, this function returns a guard that must be kept alive to ensure
    /// that all logs are flushed to disk.
    pub fn init_tracing(&self) -> eyre::Result<Option<FileWorkerGuard>> {
        let mut layers = vec![reth_tracing::stdout(
            self.verbosity.directive(),
            &self.logs.color.to_string(),
        )];

        let (additional_layers, guard) = self.logs.layers()?;
        layers.extend(additional_layers);

        reth_tracing::init(layers);
        Ok(guard)
    }

    /// Configures the given node extension.
    pub fn with_node_extension<C>(mut self, conf: C) -> Self
    where
        C: Into<Ext::Node>,
    {
        self.command.set_node_extension(conf.into());
        self
    }
}

/// Convenience function for parsing CLI options, set up logging and run the chosen command.
#[inline]
pub fn run() -> eyre::Result<()> {
    Cli::<()>::parse().run()
}

/// Commands to be executed
#[derive(Debug, Subcommand)]
pub enum Commands<Ext: RethCliExt = ()> {
    /// Start the node
    #[command(name = "node")]
    Node(node::NodeCommand<Ext>),
    /// Initialize the database from a genesis file.
    #[command(name = "init")]
    Init(init_cmd::InitCommand),
    /// This syncs RLP encoded blocks from a file.
    #[command(name = "import")]
    Import(import::ImportCommand),
    /// Database debugging utilities
    #[command(name = "db")]
    Db(db::Command),
    /// Manipulate individual stages.
    #[command(name = "stage")]
    Stage(stage::Command),
    /// P2P Debugging utilities
    #[command(name = "p2p")]
    P2P(p2p::Command),
    /// Generate Test Vectors
    #[command(name = "test-vectors")]
    TestVectors(test_vectors::Command),
    /// Write config to stdout
    #[command(name = "config")]
    Config(config_cmd::Command),
    /// Various debug routines
    #[command(name = "debug")]
    Debug(debug_cmd::Command),
    /// Scripts for node recovery
    #[command(name = "recover")]
    Recover(recover::Command),
}

impl<Ext: RethCliExt> Commands<Ext> {
    /// Sets the node extension if it is the [NodeCommand](node::NodeCommand).
    ///
    /// This is a noop if the command is not the [NodeCommand](node::NodeCommand).
    pub fn set_node_extension(&mut self, ext: Ext::Node) {
        if let Commands::Node(command) = self {
            command.ext = ext
        }
    }
}

/// The verbosity settings for the cli.
#[derive(Debug, Copy, Clone, Args)]
#[command(next_help_heading = "Display")]
pub struct Verbosity {
    /// Set the minimum log level.
    ///
    /// -v      Errors
    /// -vv     Warnings
    /// -vvv    Info
    /// -vvvv   Debug
    /// -vvvvv  Traces (warning: very verbose!)
    #[clap(short, long, action = ArgAction::Count, global = true, default_value_t = 3, verbatim_doc_comment, help_heading = "Display")]
    verbosity: u8,

    /// Silence all log output.
    #[clap(
        long,
        alias = "silent",
        short = 'q',
        global = true,
        help_heading = "Display"
    )]
    quiet: bool,
}

impl Verbosity {
    /// Get the corresponding [Directive] for the given verbosity, or none if the verbosity
    /// corresponds to silent.
    pub fn directive(&self) -> Directive {
        if self.quiet {
            LevelFilter::OFF.into()
        } else {
            let level = match self.verbosity - 1 {
                0 => Level::ERROR,
                1 => Level::WARN,
                2 => Level::INFO,
                3 => Level::DEBUG,
                _ => Level::TRACE,
            };

            format!("{level}").parse().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use crate::args::{utils::SUPPORTED_CHAINS, ColorMode};

    use super::*;

    #[test]
    fn parse_color_mode() {
        let reth = Cli::<()>::try_parse_from(["reth", "node", "--color", "always"]).unwrap();
        assert_eq!(reth.logs.color, ColorMode::Always);
    }

    /// Tests that the help message is parsed correctly. This ensures that clap args are configured
    /// correctly and no conflicts are introduced via attributes that would result in a panic at
    /// runtime
    #[test]
    fn test_parse_help_all_subcommands() {
        let reth = Cli::<()>::command();
        for sub_command in reth.get_subcommands() {
            let err = Cli::<()>::try_parse_from(["reth", sub_command.get_name(), "--help"])
                .err()
                .unwrap_or_else(|| {
                    panic!("Failed to parse help message {}", sub_command.get_name())
                });

            // --help is treated as error, but
            // > Not a true "error" as it means --help or similar was used. The help message will be sent to stdout.
            assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        }
    }

    /// Tests that the log directory is parsed correctly. It's always tied to the specific chain's
    /// name
    #[test]
    fn parse_logs_path() {
        let mut reth = Cli::<()>::try_parse_from(["reth", "node"]).unwrap();
        reth.logs.log_file_directory = reth
            .logs
            .log_file_directory
            .join(reth.chain.chain.to_string());
        let log_dir = reth.logs.log_file_directory;
        let end = format!("reth/logs/{}", SUPPORTED_CHAINS[0]);
        assert!(log_dir.as_ref().ends_with(end), "{:?}", log_dir);

        let mut iter = SUPPORTED_CHAINS.iter();
        iter.next();
        for chain in iter {
            let mut reth = Cli::<()>::try_parse_from(["reth", "node", "--chain", chain]).unwrap();
            reth.logs.log_file_directory = reth
                .logs
                .log_file_directory
                .join(reth.chain.chain.to_string());
            let log_dir = reth.logs.log_file_directory;
            let end = format!("reth/logs/{}", chain);
            assert!(log_dir.as_ref().ends_with(end), "{:?}", log_dir);
        }
    }

    #[test]
    fn override_trusted_setup_file() {
        // We already have a test that asserts that this has been initialized,
        // so we cheat a little bit and check that loading a random file errors.
        let reth = Cli::<()>::try_parse_from(["reth", "node", "--trusted-setup-file", "README.md"])
            .unwrap();
        assert!(reth.run().is_err());
    }

    #[test]
    fn parse_env_filter_directives() {
        let temp_dir = tempfile::tempdir().unwrap();

        std::env::set_var("RUST_LOG", "info,evm=debug");
        let reth = Cli::<()>::try_parse_from([
            "reth",
            "init",
            "--datadir",
            temp_dir.path().to_str().unwrap(),
            "--log.file.filter",
            "debug,net=trace",
        ])
        .unwrap();
        assert!(reth.run().is_ok());
    }
}
