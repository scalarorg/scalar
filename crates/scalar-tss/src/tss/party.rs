use super::signer::TssSigner;
use crate::storage::TssStore;
use anemo::{Network, PeerId};
use anyhow::anyhow;
use narwhal_config::{Authority, Committee};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;
use tokio::task::JoinHandle;
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
    committee: Committee,
    network: Network,
    tss_store: TssStore,
    tss_keygen: TssKeyGenerator,
    tss_signer: TssSigner,
    tx_keygen: UnboundedSender<MessageIn>,
    tx_sign: UnboundedSender<MessageIn>,
    tx_sign_result: UnboundedSender<(SignInit, SignResult)>,
}
impl TssParty {
    pub fn new(
        authority: Authority,
        committee: Committee,
        network: Network,
        tss_store: TssStore,
        tss_keygen: TssKeyGenerator,
        tss_signer: TssSigner,
        tx_keygen: UnboundedSender<MessageIn>,
        tx_sign: UnboundedSender<MessageIn>,
        tx_sign_result: UnboundedSender<(SignInit, SignResult)>,
    ) -> Self {
        Self {
            authority,
            committee,
            network,
            tss_store,
            tss_keygen,
            tss_signer,
            tx_keygen,
            tx_sign,
            tx_sign_result,
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
        tx_keygen_result: watch::Sender<Option<()>>,
        rx_sign: UnboundedReceiver<MessageIn>,
        mut rx_sign_init: UnboundedReceiver<SignInit>,
        mut rx_shutdown: ConditionalBroadcastReceiver,
    ) -> Vec<JoinHandle<()>> {
        //Init genkey protocol
        let (tx_message_out, mut rx_message_out) = mpsc::unbounded_channel();

        let keygen_init = self.tss_keygen.create_keygen_init();
        let uid = self.get_uid();

        let mut handles = Vec::new();
        let gg20_keygen_init = keygen_init.clone();
        let tss_store = self.tss_store.clone();

        let tx_message_out_keygen = tx_message_out.clone();
        let handle = tokio::spawn(async move {
            let gg20_service = Gg20Service::new(tss_store, true);
            let _ = gg20_service.init_mnemonic().await;
            let _ = gg20_service
                .keygen_init(gg20_keygen_init, rx_keygen, tx_message_out_keygen)
                .await;
        });

        handles.push(handle);

        let tss_store = self.tss_store.clone();
        let tx_message_out_sign = tx_message_out.clone();
        let gg20_service = Gg20Service::new(tss_store, true);

        let handle = tokio::spawn(async move {
            gg20_service
                .sign_init(rx_sign, tx_message_out_sign.clone())
                .await
                .expect("Sign should be initiated")
        });

        handles.push(handle);

        let tx_sign_result = self.tx_sign_result.clone();
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
                        info!("Keygen result {:?}", &res)
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

            tx_keygen_result
                .send(Some(()))
                .expect("Keygen result should be sent successfully");

            if !shuting_down {
                loop {
                    info!("Inside da loop from {}", uid);
                    tokio::select! {
                        _ = rx_shutdown.receiver.recv() => {
                            warn!("Node is shuting down");
                            // shuting_down = true;
                            break;
                        },
                        Some(sign_init) = rx_sign_init.recv() => {
                            info!("Received sign init {:?} from {}", &sign_init, uid);
                            match tss_signer.sign_execute_v2(&mut rx_message_out, &sign_init).await {
                                Ok(sign_result) => {
                                    info!("Sign result {:?}", &sign_result);
                                    let _ = tx_sign_result.send((sign_init, sign_result));
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
