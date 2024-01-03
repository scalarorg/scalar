use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_config::NodeConfig;

use sui_types::crypto::{
    get_key_pair_from_rng, AccountKeyPair, AuthorityKeyPair, NetworkKeyPair, SuiKeyPair,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct GrpcConfig {}
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ScalarNodeConfig {
    pub node_config: NodeConfig,
    #[serde(default = "default_grpc_consensus_address")]
    pub grpc_consensus_address: SocketAddr,
}

impl ScalarNodeConfig {}

pub fn default_grpc_consensus_address() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 9090)
}
