use anemo::Network;
use narwhal_config::{Authority, Committee};
use narwhal_types::ConditionalBroadcastReceiver;
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    message_out::SignResult,
    storage::TssStore,
    tss_peer_server::{TssPeer, TssPeerServer},
    MessageIn, SignInit, TssPeerService,
};

use super::{keygen::TssKeyGenerator, party::TssParty, signer::TssSigner};

// TODO: Update the builder when keygen, signer and party are refactored
pub struct PartyConfig {
    tx_keygen: mpsc::UnboundedSender<MessageIn>,
    rx_keygen: mpsc::UnboundedReceiver<MessageIn>,
    tx_sign: mpsc::UnboundedSender<MessageIn>,
    rx_sign: mpsc::UnboundedReceiver<MessageIn>,
    rx_sign_init: mpsc::UnboundedReceiver<SignInit>,
    tx_sign_result: mpsc::UnboundedSender<(SignInit, SignResult)>,
}

pub struct PartyBuilder {
    authority: Authority,
    committee: Committee,
    tss_store: TssStore,
}

impl PartyBuilder {
    pub fn new(authority: Authority, committee: Committee, tss_store: TssStore) -> Self {
        Self {
            authority,
            committee,
            tss_store,
        }
    }

    pub fn build(
        self,
    ) -> (
        InternalPartyBuilder,
        TssPeerServer<impl TssPeer>,
        mpsc::UnboundedSender<SignInit>,
        mpsc::UnboundedReceiver<(SignInit, SignResult)>,
    ) {
        // Channel for communication between Tss spawn and Anemo Tss service
        let (tx_keygen, rx_keygen) = mpsc::unbounded_channel();
        let (tx_sign, rx_sign) = mpsc::unbounded_channel();
        // Send sign init from Scalar Event handler to Tss spawn
        let (tx_sign_init, rx_sign_init) = mpsc::unbounded_channel();
        // Send sign result from tss spawn to Scalar handler
        let (tx_sign_result, rx_sign_result) = mpsc::unbounded_channel();

        let config = PartyConfig {
            rx_keygen,
            rx_sign,
            rx_sign_init,
            tx_keygen: tx_keygen.clone(),
            tx_sign: tx_sign.clone(),
            tx_sign_result,
        };

        (
            InternalPartyBuilder {
                authority: self.authority,
                committee: self.committee,
                tss_store: self.tss_store,
                config,
            },
            TssPeerServer::new(TssPeerService::new(tx_keygen, tx_sign)),
            tx_sign_init,
            rx_sign_result,
        )
    }
}

pub struct InternalPartyBuilder {
    authority: Authority,
    committee: Committee,
    tss_store: TssStore,
    config: PartyConfig,
}

impl InternalPartyBuilder {
    pub fn build(&self, network: Network) -> TssParty {
        let InternalPartyBuilder {
            authority,
            committee,
            tss_store,
            config,
        } = self;

        let PartyConfig {
            tx_keygen,
            tx_sign,
            tx_sign_result,
            ..
        } = &config;

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

        TssParty::new(
            authority.clone(),
            committee.clone(),
            network.clone(),
            tss_store.clone(),
            tss_keygen,
            tss_signer,
            tx_keygen.clone(),
            tx_sign.clone(),
            tx_sign_result.clone(),
        )
    }

    pub fn start(
        self,
        network: Network,
        rx_shutdown: ConditionalBroadcastReceiver,
    ) -> Vec<JoinHandle<()>> {
        let party = self.build(network);
        let PartyConfig {
            rx_keygen,
            rx_sign,
            rx_sign_init,
            ..
        } = self.config;

        party.run(rx_keygen, rx_sign, rx_sign_init, rx_shutdown)
    }
}
