use super::signer::TssSigner;
use crate::message_out::KeygenResult;
use crate::storage::TssStore;
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
    tss_store: TssStore,
    tss_keygen: TssKeyGenerator,
    tss_signer: TssSigner,
}

impl TssParty {
    pub fn new(
        authority: Authority,
        tss_store: TssStore,
        tss_keygen: TssKeyGenerator,
        tss_signer: TssSigner,
    ) -> Self {
        Self {
            authority,
            tss_store,
            tss_keygen,
            tss_signer,
        }
    }

    pub fn get_uid(&self) -> String {
        PeerId(self.authority.network_key().0.to_bytes()).to_string()
    }
}
impl TssParty {
    pub fn run(
        &self,
        rx_keygen: UnboundedReceiver<MessageIn>,
        tx_keygen_result: watch::Sender<Option<KeygenResult>>,
        rx_sign: UnboundedReceiver<MessageIn>,
        tx_sign_result: watch::Sender<Option<SignResult>>,
        mut rx_sign_init: UnboundedReceiver<SignInit>,
        mut rx_shutdown: ConditionalBroadcastReceiver,
    ) -> Vec<JoinHandle<()>> {
        //Init keygen protocol
        let (tx_message_out, mut rx_message_out) = mpsc::unbounded_channel();

        let keygen_init = self.tss_keygen.create_keygen_init();
        let uid = self.get_uid();

        let mut handles = Vec::new();
        let tss_store = self.tss_store.clone();
        let gg20_service = Gg20Service::new(tss_store, true);

        let gg20_keygen_init = keygen_init.clone();
        let tx_message_out_keygen = tx_message_out.clone();
        let gg20_service_keygen = gg20_service.clone();
        tokio::spawn(async move {
            let _ = gg20_service_keygen.init_mnemonic().await;
            let _ = gg20_service_keygen
                .keygen_init(gg20_keygen_init, rx_keygen, tx_message_out_keygen)
                .await;

            info!("Keygen protocol finished");
        });

        // handles.push(handle);

        let (tx_message_out_sign, mut rx_message_out_sign) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            if let Err(e) = gg20_service
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
        // handles.push(handle);

        let tss_keygen = self.tss_keygen.clone();
        let tss_signer = self.tss_signer.clone();

        let handle = tokio::spawn(async move {
            info!("Init TssParty node, starting keygen process");
            let mut shuting_down = false;

            tokio::select! {
                _ = rx_shutdown.receiver.recv() => {
                    warn!("Node is shuting down");
                    shuting_down = true;
                },
                keygen_result = tss_keygen.keygen_execute(keygen_init, &mut rx_message_out) => match keygen_result {
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

            info!(
                "Finished keygen process, loop for handling sign messsage. Shuting_down {}",
                shuting_down
            );

            if !shuting_down {
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
            info!(
                "TssParty node {:?} stopped by received shuting down signal",
                uid
            );
        });
        handles.push(handle);
        handles
    }
}
