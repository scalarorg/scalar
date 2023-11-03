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

#[derive(Debug, Deserialize, Serialize)]
pub struct KeygenRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct KeygenResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SignRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SignResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyRequest {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyResponse {
    pub message: String,
}

mod narwhal_types {
    // pub use types::*;
    include!(concat!(env!("OUT_DIR"), "/tss.network.TssPeer.rs"));
    include!(concat!(env!("OUT_DIR"), "/tofnd.rs"));
    include!(concat!(env!("OUT_DIR"), "/scalar.ScalarEvent.rs"));
}

pub use narwhal_types::*;
