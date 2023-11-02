// Copyright (c) Scalar org.
// SPDX-License-Identifier: Apache-2.0
#![warn(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    rust_2021_compatibility
)]

use base_types::{SequenceNumber, SuiAddress};

pub mod account_address;
pub mod accumulator;
pub mod balance;
pub mod base_types;
pub mod coin;
pub mod committee;
pub mod crypto;
pub mod digests;
pub mod dynamic_field;
pub mod effects;
pub mod epoch_data;
pub mod error;
pub mod event;
pub mod gas;
pub mod gas_algebra;
pub mod gas_coin;
pub mod gas_model;
pub mod governance;
pub mod id;
pub mod identifier;
pub mod language_storage;
pub mod message_envelope;
pub mod messages_checkpoint;
pub mod messages_consensus;
pub mod multisig;
pub mod object;
pub mod scalar_serde;
pub mod signature;
pub mod storage;
pub mod transaction;
pub mod type_resolver;
pub mod u256;
pub mod value;
pub mod zk_login_authenticator;
pub mod zk_login_util;

#[cfg(any(test, feature = "test-utils"))]
#[path = "./unit_tests/utils.rs"]
pub mod utils;

//pub use move_types::base_types;
//pub use move_types::*;
use account_address::AccountAddress;
use base_types::{ObjectID, SequenceNumber};
use language_storage::{StructTag, TypeTag};
use object::OBJECT_START_VERSION;

pub use mysten_network::multiaddr;

pub type CheckpointSequenceNumber = u64;
pub type CheckpointTimestamp = u64;

/// 0x1-- account address where Move stdlib modules are stored
/// Same as the ObjectID
pub const MOVE_STDLIB_ADDRESS: AccountAddress = AccountAddress::ONE;
pub const MOVE_STDLIB_PACKAGE_ID: ObjectID = ObjectID::from_address(MOVE_STDLIB_ADDRESS);

/// 0x2-- account address where sui framework modules are stored
/// Same as the ObjectID
pub const SUI_FRAMEWORK_ADDRESS: AccountAddress = address_from_single_byte(2);
pub const SUI_FRAMEWORK_PACKAGE_ID: ObjectID = ObjectID::from_address(SUI_FRAMEWORK_ADDRESS);

/// 0x3-- account address where sui system modules are stored
/// Same as the ObjectID
pub const SUI_SYSTEM_ADDRESS: AccountAddress = address_from_single_byte(3);
pub const SUI_SYSTEM_PACKAGE_ID: ObjectID = ObjectID::from_address(SUI_SYSTEM_ADDRESS);

/// 0xdee9-- account address where DeepBook modules are stored
/// Same as the ObjectID
pub const DEEPBOOK_ADDRESS: AccountAddress = deepbook_addr();
pub const DEEPBOOK_PACKAGE_ID: ObjectID = ObjectID::from_address(DEEPBOOK_ADDRESS);

/// 0x5: hardcoded object ID for the singleton sui system state object.
pub const SUI_SYSTEM_STATE_ADDRESS: AccountAddress = address_from_single_byte(5);
pub const SUI_SYSTEM_STATE_OBJECT_ID: ObjectID = ObjectID::from_address(SUI_SYSTEM_STATE_ADDRESS);
pub const SUI_SYSTEM_STATE_OBJECT_SHARED_VERSION: SequenceNumber = OBJECT_START_VERSION;

/// 0x6: hardcoded object ID for the singleton clock object.
pub const SUI_CLOCK_ADDRESS: AccountAddress = address_from_single_byte(6);
pub const SUI_CLOCK_OBJECT_ID: ObjectID = ObjectID::from_address(SUI_CLOCK_ADDRESS);
pub const SUI_CLOCK_OBJECT_SHARED_VERSION: SequenceNumber = OBJECT_START_VERSION;

/// 0x7: hardcode object ID for the singleton authenticator state object.
pub const SUI_AUTHENTICATOR_STATE_ADDRESS: AccountAddress = address_from_single_byte(7);
pub const SUI_AUTHENTICATOR_STATE_OBJECT_ID: ObjectID =
    ObjectID::from_address(SUI_AUTHENTICATOR_STATE_ADDRESS);

/// Return `true` if `addr` is a special system package that can be upgraded at epoch boundaries.
/// All new system package ID's must be added here.
pub fn is_system_package(addr: impl Into<AccountAddress>) -> bool {
    matches!(
        addr.into(),
        MOVE_STDLIB_ADDRESS | SUI_FRAMEWORK_ADDRESS | SUI_SYSTEM_ADDRESS | DEEPBOOK_ADDRESS
    )
}

const fn address_from_single_byte(b: u8) -> AccountAddress {
    let mut addr = [0u8; AccountAddress::LENGTH];
    addr[AccountAddress::LENGTH - 1] = b;
    AccountAddress::new(addr)
}

/// return 0x0...dee9
const fn deepbook_addr() -> AccountAddress {
    let mut addr = [0u8; AccountAddress::LENGTH];
    addr[AccountAddress::LENGTH - 2] = 0xde;
    addr[AccountAddress::LENGTH - 1] = 0xe9;
    AccountAddress::new(addr)
}

pub fn sui_framework_address_concat_string(suffix: &str) -> String {
    format!("{}{suffix}", SUI_FRAMEWORK_ADDRESS.to_hex_literal())
}
/*
 * 2023-11-02 TaiVV
 * Tam thoi comment out code lien quan toi Move
 * Se add lai vao cac package doc lap xu ly Move logic
 * Tags: SCALAR_MOVE_LANGUAGE
 */
// pub fn parse_sui_struct_tag(s: &str) -> anyhow::Result<StructTag> {
//     use move_command_line_common::types::ParsedStructType;
//     ParsedStructType::parse(s)?.into_struct_tag(&resolve_address)
// }

// pub fn parse_sui_type_tag(s: &str) -> anyhow::Result<TypeTag> {
//     use move_command_line_common::types::ParsedType;
//     ParsedType::parse(s)?.into_type_tag(&resolve_address)
// }

fn resolve_address(addr: &str) -> Option<AccountAddress> {
    match addr {
        "deepbook" => Some(DEEPBOOK_ADDRESS),
        "std" => Some(MOVE_STDLIB_ADDRESS),
        "sui" => Some(SUI_FRAMEWORK_ADDRESS),
        "sui_system" => Some(SUI_SYSTEM_ADDRESS),
        _ => None,
    }
}
