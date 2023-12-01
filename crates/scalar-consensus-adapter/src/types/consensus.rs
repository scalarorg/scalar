use scalar_types::transaction::Transaction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EthTransaction {}
impl EthTransaction {
    pub fn new() -> Self {
        Self {}
    }
}

impl From<EthTransaction> for Transaction {
    fn from(value: EthTransaction) -> Self {
        // Scalar TODO: Add implementation here
        // let transaction = Transaction {

        // };
        todo!()
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, JsonSchema, Clone, Default)]
#[serde(rename_all = "camelCase", rename = "AddTransactionResponse")]
pub struct ConsensusAddTransactionResponse {}
