use crate::send;
use crate::types::{
    message_in,
    message_out::{self, SignResult},
    tss_peer_client::TssPeerClient,
    MessageIn, MessageOut, SignInit, TrafficIn, TssAnemoDeliveryMessage, TssAnemoSignRequest,
};
use anemo::Network;
use anemo::PeerId;
use crypto::NetworkPublicKey;
use futures::future::join_all;
use narwhal_config::{Authority, Committee};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tonic::Status;
use tracing::{info, warn};
#[derive(Clone)]
pub struct TssSigner {
    pub uid: String,
    pub authority: Authority,
    pub committee: Committee,
    pub network: Network,
    pub tx_sign: UnboundedSender<MessageIn>,
}

impl TssSigner {
    pub fn new(
        authority: Authority,
        committee: Committee,
        network: Network,
        tx_sign: UnboundedSender<MessageIn>,
    ) -> Self {
        let uid = PeerId(authority.network_key().0.to_bytes()).to_string();
        Self {
            uid,
            authority,
            committee,
            network,
            tx_sign,
        }
    }

    pub async fn deliver_sign(&self, msg: &MessageOut) {
        let msg = msg.data.as_ref().expect("missing data");
        let msg = match msg {
            message_out::Data::Traffic(t) => t,
            _ => {
                panic!("msg must be traffic out");
            }
        };

        // let msg_in = MessageIn {
        //     data: Some(message_in::Data::Traffic(TrafficIn {
        //         from_party_uid: self.uid.clone(),
        //         is_broadcast: msg.is_broadcast,
        //         payload: msg.payload.clone(),
        //     })),
        // };
        // //Send to own tofnd gGpc Server
        // let _ = self.tx_sign.send(msg_in);

        //info!("Broadcast message {:?} from {:?}", msg, from);
        let mut handlers = Vec::new();
        let peers = self
            .committee
            .authorities()
            .filter(|auth| auth.id().0 != self.authority.id().0)
            .map(|auth| auth.network_key().clone())
            .collect::<Vec<NetworkPublicKey>>();
        let tss_message = TssAnemoDeliveryMessage {
            from_party_uid: self.uid.clone(),
            is_broadcast: msg.is_broadcast,
            payload: msg.payload.clone(),
        };
        //Send to other peers vis anemo network
        for peer in peers {
            let network = self.network.clone();
            let message = tss_message.clone();
            info!(
                "Deliver sign message from {:?} to peer {:?}",
                &self.uid,
                peer.to_string()
            );
            let f = move |peer| {
                let request = TssAnemoSignRequest {
                    message: message.to_owned(),
                };
                async move {
                    let result = TssPeerClient::new(peer).sign(request).await;
                    match result.as_ref() {
                        Ok(_r) => {
                            // info!("TssPeerClient sign result {:?}", r);
                            info!("TssPeerClient sign result");
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
        // handlers
    }

    pub async fn sign_execute_v2(
        &self,
        sign_server_outgoing: &mut UnboundedReceiver<Result<MessageOut, Status>>,
        sign_init: &SignInit,
    ) -> Result<SignResult, tonic::Status> {
        #[allow(unused_variables)]
        let all_share_count = sign_init.party_uids.len();
        #[allow(unused_variables)]
        let mut msg_count = 1;
        // the first outbound message is keygen init info

        info!("Send tss sign request to gRPC server");
        self.tx_sign
            .send(MessageIn {
                data: Some(message_in::Data::SignInit(sign_init.clone())),
            })
            .expect("SignInit should be sent successfully");

        let my_uid = self.uid.clone();
        let result = loop {
            match sign_server_outgoing.recv().await {
                Some(Ok(msg)) => {
                    let msg_type = msg.data.as_ref().expect("missing data");
                    match msg_type {
                        #[allow(unused_variables)] // allow unsused traffin in non malicious
                        message_out::Data::Traffic(traffic) => {
                            // in malicous case, if we are stallers we skip the message
                            #[cfg(feature = "malicious")]
                            {
                                let round = sign_round(msg_count, all_share_count, my_share_count);
                                if self.malicious_data.timeout_round == round {
                                    warn!(
                                        "{} is stalling a message in round {}",
                                        my_uid,
                                        round - 4
                                    ); // subtract keygen rounds
                                    continue; // tough is the life of the staller
                                }
                                if self.malicious_data.disrupt_round == round {
                                    warn!("{} is disrupting a message in round {}", my_uid, round);
                                    let mut t = traffic.clone();
                                    t.payload =
                                        traffic.payload[0..traffic.payload.len() / 2].to_vec();
                                    let mut m = msg.clone();
                                    m.data = Some(proto::message_out::Data::Traffic(t));
                                    self.deliver_sign(&m);
                                }
                            }
                            self.deliver_sign(&msg).await;
                        }
                        message_out::Data::SignResult(res) => {
                            info!("party [{}] sign finished with result", my_uid);
                            break Ok(res.clone());
                        }
                        message_out::Data::NeedRecover(_) => {
                            info!("party [{}] needs recover", my_uid);
                            // when recovery is needed, sign is canceled. We abort the protocol manualy instead of waiting parties to time out
                            // no worries that we don't wait for enough time, we will not be checking criminals in this case
                            // delivery.send_timeouts(0);
                            break Ok(SignResult {
                                sign_result_data: None,
                            });
                        }
                        _ => {
                            panic!("party [{}] sign error: bad outgoing message type", my_uid)
                        }
                    };
                    msg_count += 1;
                }
                Some(Err(e)) => {
                    warn!(
                        "party [{}] keygen execution was not completed due to error {:?}",
                        my_uid, e
                    );
                    return Ok(SignResult {
                        sign_result_data: None,
                    });
                }

                None => {
                    warn!(
                        "party [{}] sign execution was not completed due to abort",
                        my_uid
                    );
                    return Ok(SignResult {
                        sign_result_data: None,
                    });
                }
            }
        };
        result
    }
}
