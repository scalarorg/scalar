use clap::Parser;
use reth::cli::Cli;
use scalar_reth::scalar_ext::{ScalarCliExt, ScalarExt};
// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(all(feature = "optimism", not(test)))]
compile_error!("Cannot build the `reth` binary with the `optimism` feature flag enabled. Did you mean to build `op-reth`?");

#[cfg(not(feature = "optimism"))]
fn main() {
    if let Err(err) = run() {
        println!("Error in cli parse {:?}", err);
        std::process::exit(1);
    }
}

/// Convenience function for parsing CLI options, set up logging and run the chosen command.
#[inline]
pub fn run() -> eyre::Result<()> {
    Cli::<ScalarCliExt<ScalarExt>>::parse().run()
}
