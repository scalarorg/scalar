use crate::Round;
use config::{AuthorityIdentifier, Epoch};
use crypto::{to_intent_message, Signature};
use ethers::{
    types::{Block, H256},
    utils::keccak256,
};
use fastcrypto::{hash::Digest, signature_service::SignatureService};
use serde::{Deserialize, Serialize};
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    fmt,
    hash::Hasher,
};

//For simplicity all message convert to string using serde_json
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalMessage {
    pub message: Vec<u8>,
}

impl ExternalMessage {
    pub fn new(message: Vec<u8>) -> Self {
        //let _hash = block.hash.unwrap().0.clone();
        Self { message }
    }
    pub fn get_digest(&self) -> [u8; crypto::DIGEST_LENGTH] {
        let hash = keccak256(self.message.as_slice());
        hash
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScalarEventTransaction {
    pub payload: Vec<u8>,
    pub tss_signature: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct EventDigest(pub [u8; crypto::DIGEST_LENGTH]);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventVerify {
    pub digest: EventDigest,
    pub message: ExternalMessage,
    pub author: AuthorityIdentifier,
    pub round: Round,
    pub epoch: Epoch,
    pub signatures: HashMap<AuthorityIdentifier, Signature>,
}

impl From<EventDigest> for Digest<{ crypto::INTENT_MESSAGE_LENGTH }> {
    fn from(digest: EventDigest) -> Self {
        let intent_message = to_intent_message(EventDigest(digest.0));
        Digest {
            digest: bcs::to_bytes(&intent_message)
                .expect("Serialization message should not fail")
                .try_into()
                .expect("INTENT_MESSAGE_LENGTH is correct"),
        }
    }
}

impl From<Vec<u8>> for EventDigest {
    fn from(bytes: Vec<u8>) -> Self {
        EventDigest(bytes.try_into().expect("INTENT_MESSAGE_LENGTH is correct"))
    }
}
impl EventVerify {
    pub async fn new(
        author: AuthorityIdentifier,
        round: Round,
        epoch: Epoch,
        digest: EventDigest,
        message: ExternalMessage,
        signature_service: SignatureService<Signature, { crypto::INTENT_MESSAGE_LENGTH }>,
    ) -> Self {
        let mut signatures = HashMap::default();
        let signature = signature_service
            .request_signature(digest.clone().into())
            .await;
        signatures.insert(author.clone(), signature);
        Self {
            digest,
            message,
            author,
            round,
            epoch,
            signatures,
        }
    }
    pub fn digest(&self) -> EventDigest {
        EventDigest(self.digest.0.clone())
    }
    pub fn add_signature(&mut self, authority: &AuthorityIdentifier, signature: Signature) {
        self.signatures.insert(authority.clone(), signature);
    }
    pub fn get_signature(&self, authority: &AuthorityIdentifier) -> Option<&Signature> {
        self.signatures.get(authority)
    }
}

// impl fmt::Display for EventVerify {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
//         match self {
//             Self::V1(data) => {
//                 write!(f, "B{}({})", data.round, data.author)
//             }
//         }
//     }
// }
/// Used by the primary to request a vote from other primaries on newly produced headers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestVerifyRequest {
    pub event: EventVerify,
}

/// Used by the primary to reply to RequestVerifyRequest.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestVerifyResponse {
    pub event: Option<EventVerify>,
    // Indicates digests of missing certificates without which a vote cannot be provided.
    // pub missing: Vec<CertificateDigest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainTransaction {
    pub payload: Vec<u8>,
}
