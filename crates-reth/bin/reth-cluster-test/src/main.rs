use clap::*;
use futures::future::join_all;
use reth_cluster_test::{config::ClusterTestOpt, ClusterTest};

#[tokio::main]
async fn main() {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let options = ClusterTestOpt::parse();

    ClusterTest::run(options).await;
}
