use crate::Swarm;

pub struct LocalConsensusCluster {
    pub swarm: Swarm,
    // fullnode_url: String,
    // indexer_url: Option<String>,
    // faucet_key: AccountKeyPair,
    config_directory: tempfile::TempDir,
}

impl LocalConsensusCluster {
    #[allow(unused)]
    pub fn swarm(&self) -> &Swarm {
        &self.swarm
    }
}
