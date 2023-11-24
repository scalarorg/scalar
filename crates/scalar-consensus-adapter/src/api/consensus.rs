// Copyright (c) Scalar Org.
// SPDX-License-Identifier: Apache-2.0
// Author Vu Viet Tai
// Created: 2023, Nov 21

use crate::types::ConsensusAddTransactionResponse;
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use scalar_types::base_types::{ObjectID, SequenceNumber, TransactionDigest};
use scalar_types::scalar_serde::BigInt;
use sui_open_rpc_macros::open_rpc;

#[open_rpc(namespace = "scalar", tag = "Consensus API")]
#[rpc(server, client, namespace = "scalar")]
pub trait ConsensusApi {
    /// Return the transaction response object.
    #[method(name = "addTransaction")]
    async fn add_transaction(
        &self,
        /// the digest of the queried transaction
        digest: TransactionDigest,
        // /// options for specifying the content to be returned
        // options: Option<SuiTransactionBlockResponseOptions>,
    ) -> RpcResult<ConsensusAddTransactionResponse>;
}
