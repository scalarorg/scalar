use super::key_presence::TssKeyPresence;
use super::recover::TssRecover;
use super::signer::TssSigner;
use crate::message_out::KeygenResult;
use crate::KeygenOutput;
use anemo::PeerId;
use narwhal_config::Authority;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tonic::Status;
use tracing::{error, info, warn};
// use types::TssAnemoKeygenRequest;
use crate::types::{message_out::SignResult, MessageIn, SignInit};
use narwhal_types::ConditionalBroadcastReceiver;

// use crate::encrypted_sled::PasswordMethod;
use super::keygen::TssKeyGenerator;
use crate::service::Gg20Service;

#[derive(Clone)]
pub struct TssParty {
    authority: Authority,
    gg20_service: Gg20Service,
    pub tss_keygen: TssKeyGenerator,
    pub tss_signer: TssSigner,
    pub tss_key_presence: TssKeyPresence,
    pub tss_recover: TssRecover,
}

impl TssParty {
    pub fn new(
        authority: Authority,
        gg20_service: Gg20Service,
        tss_keygen: TssKeyGenerator,
        tss_signer: TssSigner,
        tss_key_presence: TssKeyPresence,
        tss_recover: TssRecover,
    ) -> Self {
        Self {
            authority,
            gg20_service,
            tss_keygen,
            tss_signer,
            tss_key_presence,
            tss_recover,
        }
    }

    pub fn get_uid(&self) -> String {
        PeerId(self.authority.network_key().0.to_bytes()).to_string()
    }
}
impl TssParty {
    pub async fn execute_keygen(
        &self,
        rx_keygen: UnboundedReceiver<MessageIn>,
        tx_keygen_result: watch::Sender<Option<KeygenResult>>,
        rx_shutdown: &mut ConditionalBroadcastReceiver,
    ) -> bool {
        let (tx_message_out_keygen, mut rx_message_out_keygen) = mpsc::unbounded_channel();
        let keygen_init = self.tss_keygen.create_keygen_init();

        let gg20_keygen_init = keygen_init.clone();
        let gg20_service_keygen = self.gg20_service.clone();
        tokio::spawn(async move {
            let _ = gg20_service_keygen.init_mnemonic().await;
            let _ = gg20_service_keygen
                .keygen_init(gg20_keygen_init, rx_keygen, tx_message_out_keygen)
                .await;

            info!("Keygen protocol finished");
        });

        let tss_keygen = self.tss_keygen.clone();
        let mut shuting_down = false;

        tokio::select! {
            _ = rx_shutdown.receiver.recv() => {
                warn!("Node is shuting down");
                shuting_down = true;
            },
            keygen_result = tss_keygen.keygen_execute(keygen_init, &mut rx_message_out_keygen) => match keygen_result {
                Ok(res) => {
                    info!("Keygen result");
                     tx_keygen_result.send(Some(res)).expect("Keygen result should be sent successfully");
                    //Todo: Send keygen result to Evm Relayer to update external public key
                },
                Err(e) => {
                    error!("Keygen Error {:?}", e);

                },
            }
        }
        shuting_down
    }

    pub async fn execute_sign(
        &self,
        mut rx_sign_init: UnboundedReceiver<SignInit>,
        rx_sign: UnboundedReceiver<MessageIn>,
        tx_sign_result: watch::Sender<Option<SignResult>>,
        rx_shutdown: &mut ConditionalBroadcastReceiver,
    ) {
        let (tx_message_out_sign, mut rx_message_out_sign) = mpsc::unbounded_channel();
        let gg20_service_sign = self.gg20_service.clone();
        tokio::spawn(async move {
            if let Err(e) = gg20_service_sign
                .sign_init(rx_sign, tx_message_out_sign.clone())
                .await
            {
                error!("sign failure: {:?}", e.to_string());
                // we can't handle errors in tokio threads. Log error if we are unable to send the status code to client.
                if let Err(e) =
                    tx_message_out_sign.send(Err(Status::invalid_argument(e.to_string())))
                {
                    error!("could not send error to client: {}", e.to_string());
                }
            }
            info!("Sign protocol finished");
        });

        let tss_signer = self.tss_signer.clone();
        let uid = self.get_uid();

        loop {
            tokio::select! {
                _ = rx_shutdown.receiver.recv() => {
                    warn!("Node is shuting down");
                    break;
                },
                Some(sign_init) = rx_sign_init.recv() => {
                    info!("Received sign init");
                    match tss_signer.sign_execute_v2(&mut rx_message_out_sign, &sign_init).await {
                        Ok(sign_result) => {
                            info!("Sign result {:?}", &sign_result);
                            tx_sign_result.send(Some(sign_result)).expect("Sign result should be sent successfully");
                        },
                        Err(e) => {
                            error!("Sign error {:?}", e);
                        },
                    }
                    info!("Finished sign process from {}", uid);
                }
            }
        }
    }

    pub async fn execute_recover(&self, keygen_output: KeygenOutput) {
        let keygen_init = self.tss_keygen.create_keygen_init();
        self.tss_recover
            .execute_recover(keygen_init, keygen_output)
            .await
    }

    pub async fn execute_key_presence(&self, key_uid: String) -> bool {
        self.tss_key_presence.execute_key_presence(key_uid).await
    }

    pub fn run(
        &self,
        rx_keygen: UnboundedReceiver<MessageIn>,
        tx_keygen_result: watch::Sender<Option<KeygenResult>>,
        rx_sign_init: UnboundedReceiver<SignInit>,
        rx_sign: UnboundedReceiver<MessageIn>,
        tx_sign_result: watch::Sender<Option<SignResult>>,
        mut rx_shutdown: ConditionalBroadcastReceiver,
    ) -> JoinHandle<()> {
        let party = self.clone();
        tokio::spawn(async move {
            info!("Init TssParty node, starting keygen process");

            let shuting_down = party
                .execute_keygen(rx_keygen, tx_keygen_result, &mut rx_shutdown)
                .await;

            info!(
                "Finished keygen process, loop for handling sign messsage. Shuting_down {}",
                shuting_down
            );

            if !shuting_down {
                party
                    .execute_sign(rx_sign_init, rx_sign, tx_sign_result, &mut rx_shutdown)
                    .await;
            }

            info!(
                "TssParty node {:?} stopped by received shuting down signal",
                party.get_uid()
            );
        })
    }
}
