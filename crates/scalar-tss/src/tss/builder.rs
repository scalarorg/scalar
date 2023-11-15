use anemo::Network;
use narwhal_config::{Authority, Committee};
use narwhal_types::ConditionalBroadcastReceiver;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

use crate::{
    message_out::{KeygenResult, SignResult},
    service::Gg20Service,
    storage::TssStore,
    tss_peer_server::{TssPeer, TssPeerServer},
    MessageIn, SignInit,
};

use super::{
    key_presence::TssKeyPresence, keygen::TssKeyGenerator, party::TssParty, recover::TssRecover,
    service::TssPeerService, signer::TssSigner,
};

// TODO: Update the builder when keygen, signer and party are refactored
pub struct PartyConfig {
    tx_keygen: mpsc::UnboundedSender<MessageIn>,
    rx_keygen: mpsc::UnboundedReceiver<MessageIn>,
    tx_keygen_result: watch::Sender<Option<KeygenResult>>,
    pub rx_keygen_result: watch::Receiver<Option<KeygenResult>>,
    pub tx_sign_init: mpsc::UnboundedSender<SignInit>,
    rx_sign_init: mpsc::UnboundedReceiver<SignInit>,
    tx_sign: mpsc::UnboundedSender<MessageIn>,
    rx_sign: mpsc::UnboundedReceiver<MessageIn>,
    tx_sign_result: watch::Sender<Option<SignResult>>,
    pub rx_sign_result: watch::Receiver<Option<SignResult>>,
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

    pub fn build(self) -> (UnstartedParty, TssPeerServer<impl TssPeer>) {
        // Channel for communication between Tss spawn and Anemo Tss service
        let (tx_keygen, rx_keygen) = mpsc::unbounded_channel();
        let (tx_sign, rx_sign) = mpsc::unbounded_channel();

        // Expose keygen result
        let (tx_keygen_result, rx_keygen_result) = watch::channel(None);

        // Send sign init from Scalar Event handler to Tss spawn
        let (tx_sign_init, rx_sign_init) = mpsc::unbounded_channel();

        // Send sign result from tss spawn to Scalar handler
        let (tx_sign_result, rx_sign_result) = watch::channel(None);

        let config = PartyConfig {
            rx_keygen,
            rx_sign,
            tx_keygen: tx_keygen.clone(),
            tx_sign: tx_sign.clone(),
            tx_keygen_result,
            rx_keygen_result,
            tx_sign_init,
            rx_sign_init,
            tx_sign_result,
            rx_sign_result,
        };

        let gg20_service = Gg20Service::new(self.tss_store, true);

        (
            UnstartedParty {
                authority: self.authority,
                committee: self.committee,
                gg20_service: gg20_service.clone(),
                config,
            },
            TssPeerServer::new(TssPeerService::new(tx_keygen, tx_sign, gg20_service)),
        )
    }
}

pub struct UnstartedParty {
    authority: Authority,
    committee: Committee,
    gg20_service: Gg20Service,
    config: PartyConfig,
}

impl UnstartedParty {
    pub fn get_config(&self) -> &PartyConfig {
        &self.config
    }

    pub fn build(&self, network: Network) -> TssParty {
        let UnstartedParty {
            authority,
            committee,
            gg20_service,
            config,
        } = self;

        let PartyConfig {
            tx_keygen, tx_sign, ..
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

        let tss_key_presence = TssKeyPresence::new(gg20_service.clone());

        let tss_recover = TssRecover::new(gg20_service.clone());

        TssParty::new(
            authority.clone(),
            self.gg20_service,
            tss_keygen,
            tss_signer,
            tss_key_presence,
            tss_recover,
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
            tx_keygen_result,
            rx_sign_init,
            rx_sign,
            tx_sign_result,
            ..
        } = self.config;

        party.run(
            rx_keygen,
            tx_keygen_result,
            rx_sign,
            tx_sign_result,
            rx_sign_init,
            rx_shutdown,
        )
    }
}
