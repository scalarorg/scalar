pub mod scalar;
pub use scalar::*;
use serde::{Deserialize, Serialize};

pub type KvValue = Vec<u8>;
pub type KeyReservation = String;
/// Returned from a successful `ReserveKey` command
// #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)] // disallow derive Clone, Copy
// pub struct KeyReservation {
//     pub(super) key: String,
// }
// /// kv store needs PartialEq to complare values
// impl PartialEq for KeyReservation {
//     fn eq(&self, other: &Self) -> bool {
//         self.key == other.key
//     }
// }

// #[derive(Debug)]
// pub struct ConditionalBroadcastReceiver {
//     pub receiver: broadcast::Receiver<()>,
// }

/// Used by workers to send a new batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TssAnemoDeliveryMessage {
    pub from_party_uid: String,
    pub is_broadcast: bool,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoKeygenRequest {
    pub message: TssAnemoDeliveryMessage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoKeygenResponse {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoSignRequest {
    pub message: TssAnemoDeliveryMessage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoSignResponse {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoVerifyRequest {
    pub message: TssAnemoDeliveryMessage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TssAnemoVerifyResponse {
    pub message: String,
}

pub mod tss_types {
    include!(concat!(env!("OUT_DIR"), "/tss.network.TssPeer.rs"));
    include!(concat!(env!("OUT_DIR"), "/tofnd.rs"));
    include!(concat!(env!("OUT_DIR"), "/scalar.ScalarEvent.rs"));
}

// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct RecoverRequest {
//     #[prost(message, optional, tag = "1")]
//     pub keygen_init: ::core::option::Option<KeygenInit>,
//     #[prost(message, optional, tag = "2")]
//     pub keygen_output: ::core::option::Option<KeygenOutput>,
// }
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct RecoverResponse {
//     #[prost(enumeration = "recover_response::Response", tag = "1")]
//     pub response: i32,
// }
/// Nested message and enum types in `RecoverResponse`.
// pub mod recover_response {
//     #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
//     #[repr(i32)]
//     pub enum Response {
//         Unspecified = 0,
//         Success = 1,
//         Fail = 2,
//     }
// }

// /// Key presence check types
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct KeyPresenceRequest {
//     #[prost(string, tag = "1")]
//     pub key_uid: ::prost::alloc::string::String,
//     /// SEC1-encoded compressed pub key bytes to find the right mnemonic. Latest is used, if empty.
//     #[prost(bytes = "vec", tag = "2")]
//     pub pub_key: ::prost::alloc::vec::Vec<u8>,
// }
// #[derive(Clone, PartialEq, ::prost::Message)]
// pub struct KeyPresenceResponse {
//     #[prost(enumeration = "key_presence_response::Response", tag = "1")]
//     pub response: i32,
// }
// /// Nested message and enum types in `KeyPresenceResponse`.
// pub mod key_presence_response {
//     #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
//     #[repr(i32)]
//     pub enum Response {
//         Unspecified = 0,
//         Present = 1,
//         Absent = 2,
//         Fail = 3,
//     }
// }
pub use tss_types::*;
