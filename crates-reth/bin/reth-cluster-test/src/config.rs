use dotenvy::dotenv;
use serde::Deserialize;

#[derive(derivative::Derivative, Deserialize)]
#[derivative(Debug)]
pub struct ClusterTestOpt {
    pub nodes: u8,
    pub chain: String,
    pub phrase: String,
    pub receiver_address: String,
    pub transaction_amount: u64,
}

impl ClusterTestOpt {
    pub fn parse() -> Self {
        dotenv().expect(".env file not found");

        match envy::from_env::<Self>() {
            Ok(config) => config,
            Err(e) => panic!("Couldn't read config ({})", e),
        }
    }

    pub fn phrase(&self) -> &str {
        &self.phrase
    }

    pub fn receiver_address(&self) -> &str {
        &self.receiver_address
    }

    pub fn nodes(&self) -> u8 {
        self.nodes
    }

    pub fn chain(&self) -> &str {
        &self.chain
    }

    pub fn transaction_amount(&self) -> u64 {
        self.transaction_amount
    }
}
