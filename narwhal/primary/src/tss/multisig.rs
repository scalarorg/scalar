use anemo::PeerId;
use anyhow::anyhow;
use config::{Authority, Committee};
use types::{MultisigKeygenRequest, MultisigSignRequest};
pub struct Multisig {
    authority: Authority,
    committee: Committee,
    pub_key: Option<Vec<u8>>,
}

impl Multisig {
    pub fn new(authority: Authority, committee: Committee) -> Self {
        Self {
            authority,
            committee,
            pub_key: None,
        }
    }
    pub fn set_pub_key(&mut self, pub_key: Vec<u8>) {
        self.pub_key = Some(pub_key);
    }
    pub fn create_keygen_request(&self) -> MultisigKeygenRequest {
        MultisigKeygenRequest {
            key_uid: format!("miltisig_{}", self.committee.epoch()),
            party_uid: PeerId(self.authority.network_key().0.to_bytes()).to_string(),
        }
    }
    pub fn create_sign_request(&self, msg_to_sign: Vec<u8>) -> anyhow::Result<MultisigSignRequest> {
        self.pub_key
            .as_ref()
            .ok_or_else(|| anyhow!("Missing pubkey"))
            .map(|pub_key| MultisigSignRequest {
                key_uid: format!("miltisig_{}", self.committee.epoch()),
                msg_to_sign,
                party_uid: PeerId(self.authority.network_key().0.to_bytes()).to_string(),
                pub_key: pub_key.clone(),
            })
    }
}
