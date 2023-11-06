use super::{create_tofnd_client, send};
use anemo::Network;
use anemo::PeerId;
use config::{Authority, Committee};
use crypto::NetworkPublicKey;
use fastcrypto::ed25519::Ed25519PublicKey;
use futures::future::join_all;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::Channel;
use tonic::Streaming;
use tracing::{info, warn};
use types::gg20_client::Gg20Client;
use types::{
    message_in,
    message_out::{self, KeygenResult},
    KeygenInit, MessageIn, MessageOut, SignInit, TrafficIn, TssAnemoDeliveryMessage,
    TssAnemoKeygenRequest, TssPeerClient,
};

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
    pub async fn keygen_execute(
        &self,
        keygen_server_outgoing: &mut Streaming<MessageOut>,
    ) -> Result<KeygenResult, tonic::Status> {
        let party_uids = self
            .committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>();
        let keygen_init = KeygenInit {
            new_key_uid: format!("tss_session{}", self.committee.epoch()),
            party_uids,
            party_share_counts: vec![1, 1, 1, 1],
            my_party_index: self.authority.id().0 as u32,
            threshold: 2,
        };
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
        // the first outbound message is keygen init info
        self.tx_keygen
            .send(MessageIn {
                data: Some(message_in::Data::KeygenInit(keygen_init)),
            })
            .unwrap();
        let my_uid = self.uid.clone();
        #[allow(unused_variables)]
        let mut msg_count = 1;
        let result = loop {
            match keygen_server_outgoing.message().await {
                Ok(Some(msg)) => {
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
                Ok(None) => {
                    warn!(
                        "party [{}] keygen execution was not completed due to abort",
                        my_uid
                    );
                    return Ok(KeygenResult::default());
                }

                Err(status) => {
                    warn!(
                        "party [{}] keygen execution was not completed due to connection error: {}",
                        my_uid, status
                    );
                    return Err(status);
                }
            }
        };
        info!("party [{}] keygen execution complete", my_uid);
        return result;
    }
    pub fn init_keygen(&self) {
        let party_uids = self
            .committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>();
        let keygen_init = KeygenInit {
            new_key_uid: format!("tss_session{}", self.committee.epoch()),
            party_uids,
            party_share_counts: vec![1, 1, 1, 1],
            my_party_index: self.authority.id().0 as u32,
            threshold: 2,
        };
        let keygen_msg = MessageIn {
            data: Some(message_in::Data::KeygenInit(keygen_init)),
        };
        self.tx_keygen.send(keygen_msg);
    }
    pub async fn handle_keygen_message(
        &self,
        msg: MessageOut,
    ) -> Result<Option<KeygenResult>, anyhow::Error> {
        let msg_type = msg.data.as_ref().expect("missing data");
        match msg_type {
            #[allow(unused_variables)] // allow unsused traffin in non malicious
            message_out::Data::Traffic(traffic) => {
                // in malicous case, if we are stallers we skip the message
                #[cfg(feature = "malicious")]
                {
                    let round = keygen_round(msg_count, all_share_count, my_share_count);
                    if self.malicious_data.timeout_round == round {
                        warn!("{} is stalling a message in round {}", my_uid, round);
                        continue; // tough is the life of the staller
                    }
                    if self.malicious_data.disrupt_round == round {
                        warn!("{} is disrupting a message in round {}", my_uid, round);
                        let mut t = traffic.clone();
                        t.payload = traffic.payload[0..traffic.payload.len() / 2].to_vec();
                        let mut m = msg.clone();
                        m.data = Some(proto::message_out::Data::Traffic(t));
                        self.deliver_keygen(&m).await;
                    }
                }
                self.deliver_keygen(&msg).await;
                Ok(None)
            }
            message_out::Data::KeygenResult(res) => {
                info!("party [{}] keygen finished!", &self.uid);
                Ok(Some(res.clone()))
            }
            _ => {
                panic!("keygen error: bad outgoing message type");
            }
        }
    }

    pub async fn keygen(
        &self,
        keygen_init: KeygenInit,
        rx_keygen: UnboundedReceiver<MessageIn>,
    ) -> Result<KeygenResult, tonic::Status> {
        let my_uid = PeerId(self.authority.network_key().0.to_bytes()).to_string();
        info!("{:?} Execute keygen flow {:?}", my_uid, keygen_init);
        let port = 50010 + self.authority.id().0;
        match create_tofnd_client(port).await {
            Err(e) => Err(tonic::Status::unavailable("Cannot create tofnd client")),
            Ok(mut client) => {
                let mut keygen_server_outgoing = client
                    .keygen(tonic::Request::new(UnboundedReceiverStream::new(rx_keygen)))
                    .await
                    .unwrap()
                    .into_inner();
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
                // the first outbound message is keygen init info
                self.tx_keygen
                    .send(MessageIn {
                        data: Some(message_in::Data::KeygenInit(keygen_init)),
                    })
                    .unwrap();
                #[allow(unused_variables)]
                let mut msg_count = 1;
                let result = loop {
                    match keygen_server_outgoing.message().await {
                        Ok(Some(msg)) => {
                            let msg_type = msg.data.as_ref().expect("missing data");
                            match msg_type {
                                #[allow(unused_variables)] // allow unsused traffin in non malicious
                                message_out::Data::Traffic(traffic) => {
                                    // in malicous case, if we are stallers we skip the message
                                    #[cfg(feature = "malicious")]
                                    {
                                        let round = keygen_round(
                                            msg_count,
                                            all_share_count,
                                            my_share_count,
                                        );
                                        if self.malicious_data.timeout_round == round {
                                            warn!(
                                                "{} is stalling a message in round {}",
                                                my_uid, round
                                            );
                                            continue; // tough is the life of the staller
                                        }
                                        if self.malicious_data.disrupt_round == round {
                                            warn!(
                                                "{} is disrupting a message in round {}",
                                                my_uid, round
                                            );
                                            let mut t = traffic.clone();
                                            t.payload = traffic.payload
                                                [0..traffic.payload.len() / 2]
                                                .to_vec();
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
                                    panic!(
                                        "party [{}] keygen error: bad outgoing message type",
                                        my_uid
                                    )
                                }
                            };
                            msg_count += 1;
                        }
                        Ok(None) => {
                            warn!(
                                "party [{}] keygen execution was not completed due to abort",
                                my_uid
                            );
                            return Ok(KeygenResult::default());
                        }

                        Err(status) => {
                            warn!(
                            "party [{}] keygen execution was not completed due to connection error: {}",
                            my_uid, status
                        );
                            return Err(status);
                        }
                    }
                };
                info!("party [{}] keygen execution complete", my_uid);
                return result;
            }
        }
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
        //Send to own tofnd gGpc Server
        info!(
            "Send message {:?} to the local gRPC server via channel",
            &msg_in
        );
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
            from_party_uid: self.uid.clone(),
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
                    info!("Create and send keygen request to peer");
                    let result = TssPeerClient::new(peer).keygen(request).await;
                    match result.as_ref() {
                        Ok(r) => {
                            //info!("TssPeerClient keygen result {:?}", r);
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
    }
}
