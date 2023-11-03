use super::{create_tofnd_client, send};
use anemo::Network;
use anemo::PeerId;
use anyhow::anyhow;
use config::{Authority, Committee};
use crypto::NetworkPublicKey;
use futures::future::join_all;
use tokio::sync::mpsc::{error::SendError, UnboundedReceiver, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::Response;
use tonic::Status;
use tonic::Streaming;
use tracing::{info, warn};
use types::{
    message_in,
    message_out::{self, SignResult},
    MessageIn, MessageOut, SignInit, TrafficIn, TssAnemoDeliveryMessage, TssAnemoKeygenRequest,
    TssAnemoSignRequest, TssPeerClient,
};
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

    pub async fn sign(
        &self,
        input: Option<SignInit>,
        rx_sign: UnboundedReceiver<MessageIn>,
    ) -> Result<SignResult, tonic::Status> {
        info!("Execute sign flow fow message {:?}", input);
        if input.is_none() {
            return Err(tonic::Status::unavailable("SignInit unavailable"));
        }
        let my_uid = PeerId(self.authority.network_key().0.to_bytes()).to_string();
        let sign_init = input.unwrap();
        let port = 50010 + self.authority.id().0;
        match create_tofnd_client(port).await {
            Err(e) => Err(Status::not_found("tofnd client not found")),
            Ok(mut client) => {
                info!("Call tss gRPC server for sign flow");
                let mut sign_server_outgoing = client
                    .sign(tonic::Request::new(UnboundedReceiverStream::new(rx_sign)))
                    .await
                    .unwrap()
                    .into_inner();
                info!("End Call tss gRPC server for sign flow");
                #[allow(unused_variables)]
                let all_share_count = sign_init.party_uids.len();
                #[allow(unused_variables)]
                let mut msg_count = 1;
                // the first outbound message is keygen init info
                info!("Send tss sign request to gRPC server");
                self.tx_sign
                    .send(MessageIn {
                        data: Some(message_in::Data::SignInit(sign_init)),
                    })
                    .unwrap();
                let result = loop {
                    match sign_server_outgoing.message().await {
                        Ok(Some(msg)) => {
                            let msg_type = msg.data.as_ref().expect("missing data");
                            match msg_type {
                                #[allow(unused_variables)] // allow unsused traffin in non malicious
                                message_out::Data::Traffic(traffic) => {
                                    // in malicous case, if we are stallers we skip the message
                                    #[cfg(feature = "malicious")]
                                    {
                                        let round =
                                            sign_round(msg_count, all_share_count, my_share_count);
                                        if self.malicious_data.timeout_round == round {
                                            warn!(
                                                "{} is stalling a message in round {}",
                                                my_uid,
                                                round - 4
                                            ); // subtract keygen rounds
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
                                            self.deliver_sign(&m);
                                        }
                                    }
                                    self.deliver_sign(&msg).await;
                                }
                                message_out::Data::SignResult(res) => {
                                    info!("party [{}] sign finished!", my_uid);
                                    break Ok(res.clone());
                                }
                                message_out::Data::NeedRecover(_) => {
                                    info!("party [{}] needs recover", my_uid);
                                    // when recovery is needed, sign is canceled. We abort the protocol manualy instead of waiting parties to time out
                                    // no worries that we don't wait for enough time, we will not be checking criminals in this case
                                    // delivery.send_timeouts(0);
                                    break Ok(SignResult::default());
                                }
                                _ => {
                                    panic!(
                                        "party [{}] sign error: bad outgoing message type",
                                        my_uid
                                    )
                                }
                            };
                            msg_count += 1;
                        }
                        Ok(None) => {
                            warn!(
                                "party [{}] sign execution was not completed due to abort",
                                my_uid
                            );
                            return Ok(SignResult::default());
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
                info!("party [{}] sign execution complete", my_uid);
                return result;
            }
        }
    }
    pub async fn send_sign_message(
        &self,
        data: Option<message_in::Data>,
    ) -> Result<(), SendError<MessageIn>> {
        self.tx_sign.send(MessageIn { data })
    }
    pub async fn sign_execute(
        &self,
        sign_server_outgoing: &mut Streaming<MessageOut>,
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
            .unwrap();
        let my_uid = self.uid.clone();
        let result = loop {
            match sign_server_outgoing.message().await {
                Ok(Some(msg)) => {
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
                            info!("party [{}] sign finished with result {:?}", my_uid, res);
                            break Ok(res.clone());
                        }
                        message_out::Data::NeedRecover(_) => {
                            info!("party [{}] needs recover", my_uid);
                            // when recovery is needed, sign is canceled. We abort the protocol manualy instead of waiting parties to time out
                            // no worries that we don't wait for enough time, we will not be checking criminals in this case
                            // delivery.send_timeouts(0);
                            break Ok(SignResult::default());
                        }
                        _ => {
                            panic!("party [{}] sign error: bad outgoing message type", my_uid)
                        }
                    };
                    msg_count += 1;
                }
                Ok(None) => {
                    warn!(
                        "party [{}] sign execution was not completed due to abort",
                        my_uid
                    );
                    return Ok(SignResult::default());
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
        result
    }
    pub async fn deliver_sign(&self, msg: &MessageOut) {
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
                        Ok(r) => {
                            //info!("TssPeerClient sign result {:?}", r);
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
        let results = join_all(handlers).await;
        //info!("All sign result {:?}", results);
        //handlers
    }
    pub async fn handle_tss_message(
        &self,
        msg_out: MessageOut,
    ) -> Result<Option<SignResult>, anyhow::Error> {
        let msg_type = msg_out.data.as_ref().expect("missing data");
        match msg_type {
            #[allow(unused_variables)] // allow unsused traffin in non malicious
            message_out::Data::Traffic(traffic) => {
                // in malicous case, if we are stallers we skip the message
                #[cfg(feature = "malicious")]
                {
                    let round = sign_round(msg_count, all_share_count, my_share_count);
                    if self.malicious_data.timeout_round == round {
                        warn!("{} is stalling a message in round {}", my_uid, round - 4); // subtract keygen rounds
                        continue; // tough is the life of the staller
                    }
                    if self.malicious_data.disrupt_round == round {
                        warn!("{} is disrupting a message in round {}", my_uid, round);
                        let mut t = traffic.clone();
                        t.payload = traffic.payload[0..traffic.payload.len() / 2].to_vec();
                        let mut m = msg_out.clone();
                        m.data = Some(proto::message_out::Data::Traffic(t));
                        self.deliver_sign(&m);
                    }
                }
                self.deliver_sign(&msg_out).await;
                Ok(None)
            }
            message_out::Data::SignResult(res) => {
                info!("party [{}] sign finished!", &self.uid);
                //self.handle_sign_result(res).await;
                Ok(Some(res.clone()))
            }
            message_out::Data::NeedRecover(_) => {
                // info!("party [{}] needs recover", my_uid);
                // when recovery is needed, sign is canceled. We abort the protocol manualy instead of waiting parties to time out
                // no worries that we don't wait for enough time, we will not be checking criminals in this case
                // delivery.send_timeouts(0);
                Err(anyhow!("NeedRecover"))
            }
            _ => {
                panic!("sign error: bad outgoing message type")
            }
        }
    }
    pub async fn handle_sign_result(&self, sign_init: &SignInit, sign_result: &SignResult) {
        info!("Sign result {:?}, {:?}", sign_init, sign_result);
    }
}
