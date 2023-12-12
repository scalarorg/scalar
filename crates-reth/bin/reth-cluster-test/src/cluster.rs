use anyhow::Ok;
use async_trait::async_trait;
use test_cluster::{TestCluster, TestClusterBuilder};

use crate::config::ClusterTestOpt;

pub struct ClusterFactory;

impl ClusterFactory {
    pub async fn start(
        options: &ClusterTestOpt,
    ) -> Result<Box<dyn Cluster + Sync + Send>, anyhow::Error> {
        // Ok(match &options.env {
        //     Env::NewLocal => Box::new(LocalNewCluster::start(options).await?),
        //     _ => Box::new(RemoteRunningCluster::start(options).await?),
        // })
        Ok(Box::new(LocalNewCluster::start(options).await?))
    }
}

/// Cluster Abstraction
#[async_trait]
pub trait Cluster {
    async fn start(options: &ClusterTestOpt) -> Result<Self, anyhow::Error>
    where
        Self: Sized;

    fn fullnode_url(&self) -> &str;

    fn shutdown(&self) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

/// Represents an up and running cluster deployed remotely.
pub struct RemoteRunningCluster {
    fullnode_url: String,
    config_directory: tempfile::TempDir,
}

#[async_trait]
impl Cluster for RemoteRunningCluster {
    async fn start(_options: &ClusterTestOpt) -> Result<Self, anyhow::Error> {
        // TODO: Deploy a cluster to a remote location.
        let fullnode_url = "http://localhost:8545".to_string();

        Ok(Self {
            config_directory: tempfile::tempdir()?,
            fullnode_url,
        })
    }

    fn fullnode_url(&self) -> &str {
        &self.fullnode_url
    }
}

/// Represents a local Cluster which starts per cluster test run.
pub struct LocalNewCluster {
    test_cluster: TestCluster,
    fullnode_url: String,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

#[async_trait]
impl Cluster for LocalNewCluster {
    async fn start(options: &ClusterTestOpt) -> Result<Self, anyhow::Error> {
        let mut cluster_builder = TestClusterBuilder::new();
        let mut handles = vec![];

        for instance in 1..=options.nodes {
            cluster_builder
                .instance(instance)
                .chain(options.chain.clone());
            let mut test_cluster = cluster_builder.build();

            let handle = tokio::task::spawn_blocking(move || {
                test_cluster.start().expect("Failed to start cluster");
            });

            handles.push(handle);
        }

        let test_cluster = cluster_builder.chain(options.chain.clone()).build();

        let fullnode_url = test_cluster.fullnode_url().to_string();

        Ok(Self {
            fullnode_url,
            test_cluster,
            handles,
        })
    }

    fn fullnode_url(&self) -> &str {
        &self.fullnode_url
    }

    fn shutdown(&self) -> Result<(), anyhow::Error> {
        for handle in &self.handles {
            handle.abort();
        }
        Ok(())
    }
}

// Make linter happy
#[async_trait]
impl Cluster for Box<dyn Cluster + Send + Sync> {
    async fn start(_options: &ClusterTestOpt) -> Result<Self, anyhow::Error> {
        unreachable!(
            "If we already have a boxed Cluster trait object we wouldn't have to call this function"
        );
    }
    fn fullnode_url(&self) -> &str {
        (**self).fullnode_url()
    }

    fn shutdown(&self) -> Result<(), anyhow::Error> {
        (**self).shutdown()
    }
}
