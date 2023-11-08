use anemo::Network;
use narwhal_config::{Authority, Committee};
use tokio::sync::mpsc;

use crate::{storage::TssStore, tss_keygen::TssKeyGenerator, TssParty, TssPeerService, TssSigner};

// TODO: Update the builder when keygen, signer and party are refactored
pub struct PartyBuilder {
    authority: Authority,
    committee: Committee,
    network: Network,
    tss_store: TssStore,
}

impl PartyBuilder {
    pub fn new(
        authority: Authority,
        committee: Committee,
        network: Network,
        tss_store: TssStore,
    ) -> Self {
        Self {
            authority,
            committee,
            network,
            tss_store,
        }
    }

    pub fn build(self) -> (TssParty, TssPeerService) {
        let PartyBuilder {
            authority,
            committee,
            network,
            tss_store,
        } = self;
        // Channel for communication between Tss spawn and Anemo Tss service
        let (tx_keygen, rx_keygen) = mpsc::unbounded_channel();
        let (tx_sign, rx_sign) = mpsc::unbounded_channel();
        // Send sign result from tss spawn to Scalar handler
        let (tx_tss_sign_result, rx_tss_sign_result) = mpsc::unbounded_channel();

        let tss_keygen = TssKeyGenerator::new(
            authority.clone(),
            committee.clone(),
            network.clone(),
            tx_keygen.clone(),
        );
        let tss_signer = TssSigner::new(
            authority.clone(),
            committee.clone(),
            network.clone(),
            tx_sign.clone(),
        );

        let tss_party = TssParty::new(
            authority.clone(),
            committee.clone(),
            network.clone(),
            tss_store,
            tss_keygen,
            tss_signer,
            tx_keygen.clone(),
            tx_sign.clone(),
            tx_tss_sign_result,
        );

        (tss_party, TssPeerService::new(tx_keygen, tx_sign))
    }
}
