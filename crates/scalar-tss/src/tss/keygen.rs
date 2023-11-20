use super::send;
use crate::types::{
    message_in,
    message_out::{self, KeygenResult},
    tss_peer_client::TssPeerClient,
    KeygenInit, MessageIn, MessageOut, TrafficIn, TssAnemoDeliveryMessage, TssAnemoKeygenRequest,
};
use anemo::Network;
use anemo::PeerId;
use futures::future::join_all;
use narwhal_config::{Authority, Committee};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tonic::Status;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct TssKeyGenerator {
    pub uid: String,
    pub authority: Authority,
    pub committee: Committee,
    pub network: Network,
    pub tx_keygen: UnboundedSender<MessageIn>,
}

impl TssKeyGenerator {
    pub fn new(
        authority: Authority,
        committee: Committee,
        network: Network,
        tx_keygen: UnboundedSender<MessageIn>,
    ) -> Self {
        let uid = PeerId(authority.network_key().0.to_bytes()).to_string();
        Self {
            uid,
            authority,
            committee,
            network,
            tx_keygen,
        }
    }

    pub fn get_key_uid(&self) -> String {
        format!("tss_session{}", self.committee.epoch())
    }

    /// Create a keygen init for tofnd
    pub fn create_keygen_init(&self) -> KeygenInit {
        let party_uids = self
            .committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>();

        let new_key_uid = self.get_key_uid();

        KeygenInit {
            new_key_uid,
            party_uids,
            party_share_counts: vec![1, 1, 1, 1],
            my_party_index: self.authority.id().0 as u32,
            threshold: 2,
        }
    }

    pub async fn keygen_execute(
        &self,
        keygen_init: KeygenInit,
        keygen_server_outgoing: &mut UnboundedReceiver<Result<MessageOut, Status>>,
    ) -> Result<KeygenResult, tonic::Status> {
        #[allow(unused_variables)]
        let all_share_count = {
            if keygen_init.party_share_counts.is_empty() {
                keygen_init.party_uids.len()
            } else {
                keygen_init.party_share_counts.iter().sum::<u32>() as usize
            }
        };
        #[allow(unused_variables)]
        let my_share_count = {
            if keygen_init.party_share_counts.is_empty() {
                1
            } else {
                keygen_init.party_share_counts[keygen_init.my_party_index as usize] as usize
            }
        };

        let my_uid = self.uid.clone();
        #[allow(unused_variables)]
        let mut msg_count = 1;
        let result = loop {
            match keygen_server_outgoing.recv().await {
                Some(Ok(msg)) => {
                    let msg_type = msg.data.as_ref().expect("missing data");
                    match msg_type {
                        #[allow(unused_variables)] // allow unsused traffin in non malicious
                        message_out::Data::Traffic(traffic) => {
                            // in malicous case, if we are stallers we skip the message
                            #[cfg(feature = "malicious")]
                            {
                                let round =
                                    keygen_round(msg_count, all_share_count, my_share_count);
                                if self.malicious_data.timeout_round == round {
                                    warn!("{} is stalling a message in round {}", my_uid, round);
                                    continue; // tough is the life of the staller
                                }
                                if self.malicious_data.disrupt_round == round {
                                    warn!("{} is disrupting a message in round {}", my_uid, round);
                                    let mut t = traffic.clone();
                                    t.payload =
                                        traffic.payload[0..traffic.payload.len() / 2].to_vec();
                                    let mut m = msg.clone();
                                    m.data = Some(proto::message_out::Data::Traffic(t));
                                    self.deliver_keygen(&m).await;
                                }
                            }
                            self.deliver_keygen(&msg).await;
                        }
                        message_out::Data::KeygenResult(res) => {
                            info!("party [{}] keygen finished!", my_uid);
                            break Ok(res.clone());
                        }
                        _ => {
                            panic!("party [{}] keygen error: bad outgoing message type", my_uid)
                        }
                    };
                    msg_count += 1;
                }
                Some(Err(e)) => {
                    warn!(
                        "party [{}] keygen execution was not completed due to error {:?}",
                        my_uid, e
                    );
                    return Ok(KeygenResult {
                        keygen_result_data: None,
                    });
                }
                None => {
                    warn!(
                        "party [{}] keygen execution was not completed due to abort",
                        my_uid
                    );
                    return Ok(KeygenResult {
                        keygen_result_data: None,
                    });
                }
            }
        };
        info!("party [{}] keygen execution complete", my_uid);

        result
    }

    pub async fn deliver_keygen(&self, msg: &MessageOut) {
        let msg = msg.data.as_ref().expect("missing data");
        let msg = match msg {
            message_out::Data::Traffic(t) => t,
            _ => {
                panic!("msg must be traffic out");
            }
        };
        let msg_in = MessageIn {
            data: Some(message_in::Data::Traffic(TrafficIn {
                from_party_uid: self.uid.clone(),
                is_broadcast: msg.is_broadcast,
                payload: msg.payload.clone(),
            })),
        };
        // Send received message to the local gg20 Service via channel
        let _ = self.tx_keygen.send(msg_in);
        let mut handlers = Vec::new();

        // let peers = self
        //     .committee
        //     .authorities()
        //     .filter(|auth| auth.id().0 != self.authority.id().0)
        //     .map(|auth| auth.network_key().clone())
        //     .collect::<Vec<NetworkPublicKey>>();

        let peers = self.network.peers();

        let gg20_message = TssAnemoDeliveryMessage {
            from_party_uid: self.uid.clone(),
            is_broadcast: msg.is_broadcast,
            payload: msg.payload.clone(),
        };

        //Send to other peers vis anemo network
        for peer in peers {
            let network = self.network.clone();
            let message = gg20_message.clone();
            let f = move |peer| {
                let request = TssAnemoKeygenRequest {
                    message: message.to_owned(),
                };
                async move {
                    // Create and send keygen request to peer
                    let result = TssPeerClient::new(peer).keygen(request).await;
                    match result.as_ref() {
                        Ok(_r) => {
                            //info!("Gg0PeerClient keygen result {:?}", r);
                        }
                        Err(e) => {
                            error!("Gg0PeerClient keygen error {:?}", e);
                        }
                    }
                    result
                }
            };

            let handle = send(network, peer, f);
            handlers.push(handle);
        }
        let _results = join_all(handlers).await;
    }
}
