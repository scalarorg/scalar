use super::signer::TssSigner;
use crate::send;
use crate::storage::TssStore;
use crate::types::message_out::keygen_result::KeygenResultData;
use anemo::{Network, PeerId};
use anyhow::anyhow;
use crypto::NetworkPublicKey;
use futures::future::join_all;
use k256::ecdsa::hazmat::VerifyPrimitive;
use k256::elliptic_curve::sec1::FromEncodedPoint;
use k256::elliptic_curve::ScalarPrimitive;
use k256::EncodedPoint;
use k256::ProjectivePoint;
use narwhal_config::{Authority, Committee};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
// use types::TssAnemoKeygenRequest;
use crate::types::{
    message_in,
    message_out::sign_result::SignResultData,
    message_out::SignResult,
    message_out::{self},
    tss_peer_client::TssPeerClient,
    MessageIn, MessageOut, SignInit, TrafficIn, TssAnemoDeliveryMessage, TssAnemoKeygenRequest,
    TssAnemoSignRequest,
};
use narwhal_types::ConditionalBroadcastReceiver;

// use crate::encrypted_sled::PasswordMethod;
use crate::service::Gg20Service;
use crate::tss_keygen::TssKeyGenerator;

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
    tx_tss_sign_result: UnboundedSender<(SignInit, SignResult)>,
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
        tx_tss_sign_result: UnboundedSender<(SignInit, SignResult)>,
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
            tx_tss_sign_result,
        }
    }

    pub fn get_uid(&self) -> String {
        PeerId(self.authority.network_key().0.to_bytes()).to_string()
    }

    pub fn get_parties(&self) -> Vec<String> {
        let party_uids = self
            .committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>();
        party_uids
    }

    pub async fn deliver_keygen(&self, msg: &MessageOut, from: &str) {
        let msg = msg.data.as_ref().expect("missing data");
        let msg = match msg {
            message_out::Data::Traffic(t) => t,
            _ => {
                panic!("msg must be traffic out");
            }
        };
        let msg_in = MessageIn {
            data: Some(message_in::Data::Traffic(TrafficIn {
                from_party_uid: from.to_string(),
                is_broadcast: msg.is_broadcast,
                payload: msg.payload.clone(),
            })),
        };
        //Send to own tofnd gGpc Server
        let _ = self.tx_keygen.send(msg_in);
        //info!("Broadcast message {:?} from {:?}", msg, from);
        let mut handlers = Vec::new();
        let peers = self
            .committee
            .authorities()
            .filter(|auth| auth.id().0 != self.authority.id().0)
            .map(|auth| auth.network_key().clone())
            .collect::<Vec<NetworkPublicKey>>();
        let tss_message = TssAnemoDeliveryMessage {
            from_party_uid: from.to_string(),
            is_broadcast: msg.is_broadcast,
            payload: msg.payload.clone(),
        };
        //Send to other peers vis anemo network
        for peer in peers {
            let network = self.network.clone();
            let message = tss_message.clone();
            // info!(
            //     "Deliver keygen message from {:?} to peer {:?}",
            //     from,
            //     peer.to_string()
            // );
            let f = move |peer| {
                let request = TssAnemoKeygenRequest {
                    message: message.to_owned(),
                };
                async move {
                    let result = TssPeerClient::new(peer).keygen(request).await;
                    match result.as_ref() {
                        Ok(r) => {
                            info!("TssPeerClient keygen result {:?}", r);
                        }
                        Err(e) => {
                            info!("TssPeerClient keygen error {:?}", e);
                        }
                    }
                    result
                }
            };

            let handle = send(network, peer, f);
            handlers.push(handle);
        }
        let _results = join_all(handlers).await;
        //info!("All keygen result {:?}", results);
        //handlers
    }

    pub async fn deliver_sign(&self, msg: &MessageOut, from: &str) {
        let msg = msg.data.as_ref().expect("missing data");
        let msg = match msg {
            message_out::Data::Traffic(t) => t,
            _ => {
                panic!("msg must be traffic out");
            }
        };
        let msg_in = MessageIn {
            data: Some(message_in::Data::Traffic(TrafficIn {
                from_party_uid: from.to_string(),
                is_broadcast: msg.is_broadcast,
                payload: msg.payload.clone(),
            })),
        };
        //Send to own tofnd gGpc Server
        let _ = self.tx_sign.send(msg_in);
        //info!("Broadcast message {:?} from {:?}", msg, from);
        let mut handlers = Vec::new();
        let peers = self
            .committee
            .authorities()
            .filter(|auth| auth.id().0 != self.authority.id().0)
            .map(|auth| auth.network_key().clone())
            .collect::<Vec<NetworkPublicKey>>();
        let tss_message = TssAnemoDeliveryMessage {
            from_party_uid: from.to_string(),
            is_broadcast: msg.is_broadcast,
            payload: msg.payload.clone(),
        };
        //Send to other peers vis anemo network
        for peer in peers {
            let network = self.network.clone();
            let message = tss_message.clone();
            info!(
                "Deliver sign message from {:?} to peer {:?}",
                from,
                peer.to_string()
            );
            let f = move |peer| {
                let request = TssAnemoSignRequest {
                    message: message.to_owned(),
                };
                async move {
                    let result = TssPeerClient::new(peer).sign(request).await;
                    match result.as_ref() {
                        Ok(r) => {
                            info!("TssPeerClient sign result {:?}", r);
                        }
                        Err(e) => {
                            info!("TssPeerClient sign error {:?}", e);
                        }
                    }
                    result
                }
            };

            let handle = send(network, peer, f);
            handlers.push(handle);
        }
        let _results = join_all(handlers).await;
        //info!("All sign result {:?}", results);
        //handlers
    }

    pub async fn set_keygen(&mut self, key_data: KeygenResultData) {
        // info!("Keygen result {:?}", &key_data);
        info!("Keygen result");
        match key_data {
            KeygenResultData::Data(_data) => {
                //self.tss_store.write().await.set_key(data);
            }
            KeygenResultData::Criminals(_c) => {
                // warn!("Crimials {:?}", c);
                warn!("Crimials");
            }
        }
    }
    pub async fn verify_sign_result(
        &mut self,
        _message_digest: Vec<u8>,
        sign_data: SignResultData,
    ) {
        // info!("Sign result data {:?}", &sign_data);
        info!("Sign result data");
        match sign_data {
            SignResultData::Signature(sig) => {
                info!("Vefifying signature {:?}", sig.as_slice());
                // let pub_key = self.tss_store.read().await.get_key();
                // match pub_key {
                //     Some(key) => {
                //         info!("pub key {:?}", &key.pub_key);
                //         let verify_result = self.verify(
                //             key.pub_key.as_slice(),
                //             message_digest.as_slice(),
                //             sig.as_slice(),
                //         );
                //         info!("Verify result {:?}", verify_result);
                //     }
                //     None => warn!("Missing pubkey"),
                // }
            }
            SignResultData::Criminals(_c) => {
                // warn!("Crimials {:?}", c);
                warn!("Crimials");
            }
        }
    }
    fn verify(&self, pub_key: &[u8], message: &[u8], signature: &[u8]) -> anyhow::Result<bool> {
        let signature = k256::ecdsa::Signature::from_der(signature)
            .map_err(|_| anyhow!("Invalid signature"))?;
        let scalar = ScalarPrimitive::from_slice(message)?;
        let _hashed_msg = k256::Scalar::from(scalar);
        let prj_point =
            ProjectivePoint::from_encoded_point(&EncodedPoint::from_bytes(pub_key)?).unwrap();
        let res = prj_point
            .to_affine()
            .verify_prehashed(message.into(), &signature)
            .is_ok();
        Ok(res)
    }
}
impl TssParty {
    pub fn run_v2(
        &self,
        rx_keygen: UnboundedReceiver<MessageIn>,
        _rx_sign: UnboundedReceiver<MessageIn>,
        _rx_message_out: UnboundedReceiver<MessageOut>,
        mut rx_sign_init: UnboundedReceiver<SignInit>,
        mut rx_shutdown: ConditionalBroadcastReceiver,
    ) -> Vec<JoinHandle<()>> {
        //Init genkey protocol
        let (tx_message_out, mut rx_message_out) = mpsc::unbounded_channel();

        let keygen_init = self.tss_keygen.create_keygen_init();
        let uid = self.get_uid();
        // let tofnd_path = format!("/tss/.tofnd{}", self.authority.id().0);
        // info!("Init kvManager in dir {}", &tofnd_path);
        let mut handles = Vec::new();
        let gg20_keygen_init = keygen_init.clone();
        let tss_store = self.tss_store.clone();
        let handle = tokio::spawn(async move {
            //Start gg20 service with kv_manager
            // let config: Config = Config {
            //     //tofnd_path: tofnd_path.into(),
            //     password_method: PasswordMethod::NoPassword,
            //     safe_keygen: true,
            // };
            let gg20_service = Gg20Service::new(tss_store, true);
            let _ = gg20_service.init_mnemonic().await;
            let _ = gg20_service
                .keygen_init(gg20_keygen_init, rx_keygen, tx_message_out)
                .await;
            // match Gg20Service::new(config).await {
            //     Ok(gg20_service) => {
            //         let _ = gg20_service
            //             .keygen_init(gg20_keygen_init, rx_keygen, tx_message_out)
            //             .await;
            //     }
            //     Err(e) => panic!("{:?}", e),
            // }
        });
        handles.push(handle);
        let tx_sign_result = self.tx_tss_sign_result.clone();
        let tss_keygen = self.tss_keygen.clone();
        let tss_signer = self.tss_signer.clone();
        let handle = tokio::spawn(async move {
            info!("Init TssParty node, starting keygen process");
            let mut shuting_down = false;
            //let mut keygened = false;
            tokio::select! {
                _ = rx_shutdown.receiver.recv() => {
                    warn!("Node is shuting down");
                    shuting_down = true;
                },
                keygen_result = tss_keygen.keygen_execute_v2(keygen_init, &mut rx_message_out) => match keygen_result {
                    Ok(_res) => {
                        // info!("Keygen result {:?}", &res);
                        // info!("Keygen result {:?}", &res);
                        info!("Keygen result");
                        //Todo: Send keygen result to Evm Relayer to update external public key
                        // keygened = true;
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
                            shuting_down = true;
                            break;
                        },
                        Some(sign_init) = rx_sign_init.recv() => {
                            // info!("Received sign init {:?}", &sign_init);
                            info!("Received sign init");
                            match tss_signer.sign_execute_v2(&mut rx_message_out, &sign_init).await {
                                Ok(sign_result) => {
                                    // info!("Sign result {:?}", &sign_result);
                                    info!("Sign result");
                                    let _ = tx_sign_result.send((sign_init, sign_result));
                                },
                                Err(e) => {
                                    error!("Sign error {:?}", e);
                                },
                            }
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

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn spawn_v2(
        authority: Authority,
        committee: Committee,
        network: Network,
        tss_store: TssStore,
        tx_keygen: UnboundedSender<MessageIn>,
        rx_keygen: UnboundedReceiver<MessageIn>,
        tx_sign: UnboundedSender<MessageIn>,
        rx_sign: UnboundedReceiver<MessageIn>,
        rx_message_out: UnboundedReceiver<MessageOut>,
        rx_tss_sign_init: UnboundedReceiver<SignInit>,
        tx_tss_sign_result: UnboundedSender<(SignInit, SignResult)>,
        rx_shutdown: ConditionalBroadcastReceiver,
    ) -> Vec<JoinHandle<()>> {
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
            tx_keygen,
            tx_sign,
            tx_tss_sign_result,
        );
        tss_party.run_v2(
            rx_keygen,
            rx_sign,
            rx_message_out,
            rx_tss_sign_init,
            rx_shutdown,
        )
    }
}
