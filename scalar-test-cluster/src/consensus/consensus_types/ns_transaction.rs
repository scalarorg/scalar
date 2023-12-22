/*
 * 2023-12-13 Taivv
 * Defined transaction with namespace for different system
 */

use narwhal_types::Transaction;
use serde::{Deserialize, Serialize};

use crate::ExternalTransaction;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NsTransaction {
    pub namespace: String,
    #[serde(with = "serde_bytes")]
    pub transaction: Transaction,
}
impl NsTransaction {
    pub fn new(namespace: String, transaction: Transaction) -> NsTransaction {
        Self {
            namespace,
            transaction,
        }
    }
}

impl Into<ExternalTransaction> for NsTransaction {
    fn into(self) -> ExternalTransaction {
        let NsTransaction {
            namespace,
            transaction,
        } = self;
        ExternalTransaction {
            namespace,
            tx_bytes: transaction,
        }
    }
}

impl From<ExternalTransaction> for NsTransaction {
    fn from(value: ExternalTransaction) -> Self {
        let ExternalTransaction {
            namespace,
            tx_bytes,
        } = value;

        NsTransaction {
            namespace,
            transaction: tx_bytes,
        }
    }
}
