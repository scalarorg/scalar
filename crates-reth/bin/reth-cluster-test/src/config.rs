// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use clap::*;

#[derive(Parser, Clone, ValueEnum, Debug)]
pub enum Env {
    Devnet,
    Staging,
    Ci,
    CiNomad,
    Testnet,
    CustomRemote,
    NewLocal,
}

#[derive(derivative::Derivative, Parser)]
#[derivative(Debug)]
#[clap(name = "", rename_all = "kebab-case")]
pub struct ClusterTestOpt {
    #[clap(value_enum)]
    pub env: Env,
    #[clap(long)]
    pub fullnode_address: Option<String>,
    #[clap(long, default_value_t = 4)]
    pub nodes: u32,
}

impl ClusterTestOpt {
    pub fn new_local() -> Self {
        Self {
            env: Env::NewLocal,
            fullnode_address: None,
            nodes: 4,
        }
    }
}
