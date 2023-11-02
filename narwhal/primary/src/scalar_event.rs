use crate::tss::{TssParty, TssSigner};
use anemo::{Network, PeerId};
use bytes::Bytes;
use config::{committee, AuthorityIdentifier, Committee, Epoch};
use config::{Authority, WorkerCache};
use core::time::Duration;
use crypto::{KeyPair, NetworkPublicKey, Signature};
use fastcrypto::{
    hash::Digest,
    signature_service::SignatureService,
    traits::{KeyPair as _, VerifyingKey},
};
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use network::anemo_ext::NetworkExt;
use serde::{Deserialize, Serialize};
use shared_crypto::intent::{Intent, IntentScope};
use storage::EventStore;
use sui_types::transaction::{SenderSignedTransaction, TransactionData};
use sui_types::{
    base_types::AuthorityName, crypto::AuthoritySignInfo, messages_consensus::ConsensusTransaction,
    transaction::CertifiedTransaction, transaction::SenderSignedData,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio::{sync::mpsc::UnboundedReceiver, task::JoinHandle};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};
use types::error::{DagError, DagResult};
use types::{
    message_out::{sign_result::SignResultData, KeygenResult, SignResult},
    scalar_event_client::ScalarEventClient,
    scalar_event_server::{ScalarEvent, ScalarEventServer},
    ConditionalBroadcastReceiver, CrossChainTransaction, EventDigest, EventVerify, ExternalMessage,
    MessageIn, PrimaryToPrimaryClient, RequestVerifyRequest, RequestVerifyResponse, Round,
    TransactionProto, TransactionsClient,
};
use types::{MessageOut, SignInit};

#[derive(Clone, Serialize, Deserialize)]
pub struct ScalarEventTransaction {
    payload: Vec<u8>,
    signature: Vec<u8>,
}

pub struct ScalarEventHandler {
    pub primary_keypair: KeyPair,
    pub authority_id: AuthorityIdentifier,
    pub committee: Committee,
    pub worker_cache: WorkerCache,
    /// The current round of the dag.
    pub round: Round,
    pub signature_service: SignatureService<Signature, { crypto::INTENT_MESSAGE_LENGTH }>,
    pub event_store: EventStore,
    pub network: Network,
}

impl ScalarEventHandler {
    pub fn run(
        mut self,
        mut rx_external_message: UnboundedReceiver<ExternalMessage>,
        mut rx_sign_result: UnboundedReceiver<(SignInit, SignResult)>,
        mut rx_shutdown: ConditionalBroadcastReceiver,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            info!(
                "Spawn external event for node with authority {:?}",
                &self.authority_id
            );
            loop {
                tokio::select! {
                    _ = rx_shutdown.receiver.recv() => {
                        warn!("Node is shuting down");
                        break;
                    },
                    Some(msg) = rx_external_message.recv() => {
                        //Sign message and send request verification
                        info!("{:?} Receive message from external chain {:?}", &self.authority_id, &msg);
                        self.handle_external_message(msg).await;

                    },
                    Some((sign_init, sign_result)) = rx_sign_result.recv() => {
                        info!("{:?} Received SignResult from TssParty {:?} {:?}", &self.authority_id, &sign_init, &sign_result);
                        self.handle_sign_result(sign_init, sign_result).await;

                    }
                }
            }
            info!(
                "ScalarEvent handler is stopped by received shuting down signal {:?}",
                &self.authority_id
            );
        })
    }
    async fn handle_external_message(&self, msg: ExternalMessage) {
        //Block<H256>.hash
        //msg.block.hash.unwrap().0.clone()
        let digest = EventDigest(msg.get_digest());
        let round = self.round.clone();
        let committee = self.committee.clone();
        let authority = self.authority_id.clone();
        let signature_service = self.signature_service.clone();
        let event = EventVerify::new(
            authority.clone(),
            round,
            committee.epoch(),
            digest.clone(),
            msg,
            signature_service,
        )
        .await;
        if let Ok(Some(mut stored_event)) = self.event_store.read(&digest).await {
            info!("Stored event {:?}", &stored_event);
            if let Some(signature) = event.get_signature(&authority) {
                stored_event.add_signature(&authority, signature.clone());
                if let Err(e) = self.event_store.write(&stored_event).await {
                    error!("Event store error {:?}", e);
                }
            }
        } else {
            info!("Write event into node's event_store {:?}", &event);
            self.event_store.write(&event);
        }
        let network = self.network.clone();
        Self::propose_event(authority, committee, event, network).await;
    }
    async fn handle_sign_result(&self, sign_init: SignInit, sign_result: SignResult) {
        if let Some(sign_result_data) = sign_result.sign_result_data {
            match sign_result_data {
                SignResultData::Signature(sig) => {
                    let _ = self.submit_event_transaction(&sign_init, sig.clone()).await;
                }
                SignResultData::Criminals(c) => {
                    warn!("Criminals {:?}", c);
                }
            }
        }
    }
    // fn create_consensus_transaction(
    //     &self,
    //     sender_signed_trans: SenderSignedData,
    // ) -> ConsensusTransaction {
    //     let authority = self.committee.authority(&self.authority_id).unwrap();
    //     let authority_name = AuthorityName::from(authority.protocol_key());
    //     let mut signatures = Vec::new();
    //     let sig = AuthoritySignInfo::new(
    //         self.committee.epoch(),
    //         transaction.data(),
    //         Intent::sui_app(IntentScope::SenderSignedTransaction),
    //         authority_name.clone(),
    //         &self.primary_keypair,
    //     );
    //     signatures.push(sig);
    //     let certificate =
    //         CertifiedTransaction::new(sender_signed_trans, signatures, &self.committee).unwrap();
    //     ConsensusTransaction::new_certificate_message(&authority_name, certificate)
    // }
    // ConsensusTransactionBytes:
    // [83, 237, 223, 22, 186, 64, 7, 113, 1, 2, 0, 0, 0, 0, 0, 0, 0, 229, 0, 0, 0, 0, 0, 0, 0, 232, 0, 0, 0, 0, 0, 0, 0, 32, 89, 117, 113, 29, 177, 5, 78, 141, 191, 83, 17, 55, 65, 18, 85, 161, 237, 96, 114, 98, 120, 185, 169, 163, 91, 111, 28, 22, 222, 71, 85, 120, 1, 32, 210, 85, 147, 19, 197, 239, 139, 176, 181, 165, 42, 243, 17, 168, 60, 88, 210, 159, 156, 50, 55, 131, 160, 121, 187, 127, 4, 178, 96, 125, 127, 212, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 27, 48, 161, 187, 138, 1, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 96, 179, 253, 94, 251, 92, 135, 36, 3, 164, 173, 19, 241, 25, 149, 19, 96, 109, 35, 53, 74, 132, 82, 73, 174, 161, 87, 86, 76, 35, 255, 60, 21, 198, 201, 230, 82, 183, 45, 116, 14, 140, 172, 185, 60, 188, 192, 104, 107, 14, 192, 204, 12, 28, 118, 11, 251, 66, 180, 54, 81, 159, 197, 168, 5, 75, 118, 83, 58, 151, 169, 242, 57, 135, 208, 151, 239, 150, 177, 242, 82, 107, 178, 7, 63, 161, 10, 119, 25, 148, 23, 114, 52, 126, 242, 116, 244, 183, 116, 118, 69, 197, 15, 255, 163, 20, 5, 219, 111, 110, 48, 70, 59, 255, 66, 102, 162, 79, 58, 233, 2, 138, 90, 249, 179, 2, 97, 35, 229, 173, 22, 247, 16, 54, 94, 230, 48, 210, 116, 89, 99, 246, 202, 209, 231]
    // [0, 70, 48, 68, 2, 32, 92, 17, 222, 16, 213, 23, 61, 35, 107, 187, 76, 181, 247, 168, 172, 67, 107, 30, 239, 24, 99, 37, 81, 166, 121, 220, 177, 32, 152, 72, 229, 75, 2, 32, 105, 37, 82, 136, 219, 250, 240, 112, 150, 195, 205, 90, 51, 29, 192, 194, 92, 240, 240, 225, 169, 80, 16, 245, 211, 139, 244, 102, 173, 227, 244, 13]
    async fn submit_event_transaction(
        &self,
        sign_init: &SignInit,
        signature: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        info!(
            "Submit event transaction for {:?} with signature {:?}",
            sign_init, signature
        );
        let mut trans_client = self.create_remote_client();
        let digest = EventDigest::from(sign_init.message_to_sign.clone());
        if let Ok(Some(EventVerify { message, .. })) = self.event_store.read(&digest).await {
            let transaction = ScalarEventTransaction {
                payload: message.message,
                signature: signature.clone(),
            };
            let serialized =
                bcs::to_bytes(&transaction).expect("Serializing transaction cannot fail");
            // let tran_data = TransactionData::new(kind, sender, gas_payment, gas_budget, gas_price)
            // let sender_signed_trans = SenderSignedData::new_from_sender_signature(tran_data);
            // let consensus_trans = self.create_consensus_transaction(sender_signed_trans);
            // Todo: serialized data here then deserialized into ConsensusTransaction
            // let serialized =
            //     bcs::to_bytes(&consensus_trans).expect("Serializing transaction cannot fail");
            info!("EventTransaction serialized: {:?}", serialized.as_slice());
            let request = TransactionProto {
                //transaction: Bytes::from(epoch.to_be_bytes().to_vec()),
                transaction: Bytes::from(serialized.clone()),
            };
            let result = trans_client.submit_transaction(request).await;
            if result.is_ok() {
                info!("ScalarEvent::AnemoClient submit_transaction successfully");
            } else {
                debug!("ScalarEvent::AnemoClient submit_transaction failed");
            }
        } else {
            warn!(
                "Cannot find EventVerify with digest {:?} from EventStore",
                &digest
            );
        }
        Ok(())
    }
    fn create_remote_client(&self) -> TransactionsClient<Channel> {
        let pubkey = self.primary_keypair.public().clone();
        let target = self
            .worker_cache
            .worker(&pubkey, /* id */ &0)
            .expect("Our key or worker id is not in the worker cache")
            .transactions;
        let config = mysten_network::config::Config::new();
        let channel = config.connect_lazy(&target).unwrap();
        //Remote client
        TransactionsClient::new(channel)
    }
}
impl ScalarEventHandler {
    pub fn new(
        primary_keypair: KeyPair,
        authority_id: AuthorityIdentifier,
        committee: Committee,
        worker_cache: WorkerCache,
        signature_service: SignatureService<Signature, { crypto::INTENT_MESSAGE_LENGTH }>,
        event_store: EventStore,
        network: Network,
    ) -> Self {
        ScalarEventHandler {
            primary_keypair,
            authority_id,
            committee,
            worker_cache,
            round: 0,
            signature_service,
            event_store,
            network,
        }
    }
    pub fn spawn(
        primary_keypair: KeyPair,
        authority: Authority,
        committee: Committee,
        worker_cache: WorkerCache,
        signature_service: SignatureService<Signature, { crypto::INTENT_MESSAGE_LENGTH }>,
        event_store: EventStore,
        network: Network,
        rx_sign_result: UnboundedReceiver<(SignInit, SignResult)>,
        rx_external_message: UnboundedReceiver<ExternalMessage>,
        rx_shutdown: ConditionalBroadcastReceiver,
    ) -> JoinHandle<()> {
        let mut handler = ScalarEventHandler::new(
            primary_keypair,
            authority.id(),
            committee,
            worker_cache,
            signature_service,
            event_store,
            network,
        );
        handler.run(rx_external_message, rx_sign_result, rx_shutdown)
    }
    pub async fn propose_event(
        authority_id: AuthorityIdentifier,
        committee: Committee,
        event: EventVerify,
        network: anemo::Network,
    ) -> DagResult<()> {
        info!("propose_event {:?}", &event);
        let peers = committee
            .others_primaries_by_id(authority_id)
            .into_iter()
            .map(|(name, _, network_key)| (name, network_key));
        let mut requests: FuturesUnordered<_> = peers
            .map(|(name, target)| {
                let event = event.clone();
                Self::request_verify(network.clone(), committee.clone(), name, target, event)
            })
            .collect();
        loop {
            // if certificate.is_some() {
            //     break;
            // }
            tokio::select! {
                result = &mut requests.next() => {
                    match result {
                        Some(Ok(event)) => {
                            // certificate = votes_aggregator.append(
                            //     vote,
                            //     &committee,
                            //     &header,
                            // )?;
                            info!("event_verify result {:?}", &event);
                        },
                        Some(Err(e)) => debug!("failed to get vote for header {event:?}: {e:?}"),
                        None => break,
                    }
                },
                // _ = &mut cancel => {
                //     debug!("canceling Header proposal {header} for round {}", header.round());
                //     return Err(DagError::Canceled)
                // },
            }
        }
        Ok(())
    }
    async fn request_verify(
        network: anemo::Network,
        committee: Committee,
        authority: AuthorityIdentifier,
        target: NetworkPublicKey,
        event: EventVerify,
    ) -> DagResult<()> {
        let peer_id = anemo::PeerId(target.0.to_bytes());
        let peer = network.waiting_peer(peer_id);

        //let mut client = PrimaryToPrimaryClient::new(peer);
        let mut client = ScalarEventClient::new(peer);
        let request = anemo::Request::new(RequestVerifyRequest {
            event: event.clone(),
        })
        .with_timeout(Duration::from_secs(30));
        info!("Send a request_event_verify via anemo network {:?}", &event);
        match client.request_event_verify(request).await {
            Ok(response) => {
                let response = response.into_body();
                if response.event.is_some() {
                    //break response.vote.unwrap();
                }
                //missing_parents = response.missing;
            }
            Err(status) => {
                if status.status() == anemo::types::response::StatusCode::BadRequest {
                    return Err(DagError::NetworkError(format!(
                        "unrecoverable error requesting vote for {event:?}: {status:?}"
                    )));
                }
                //missing_parents = Vec::new();
            }
        }
        Ok(())
    }
}

pub struct ScalarEventService {
    committee: Committee,
    event_store: EventStore,
    tx_tss_sign: UnboundedSender<SignInit>,
}
impl ScalarEventService {
    pub fn new(
        committee: Committee,
        event_store: EventStore,
        tx_tss_sign: UnboundedSender<SignInit>,
    ) -> Self {
        Self {
            committee,
            event_store,
            tx_tss_sign,
        }
    }
    fn create_sign_init(&self, event: &EventVerify) -> SignInit {
        let party_uids = self
            .committee
            .authorities()
            .map(|authority| PeerId(authority.network_key().0.to_bytes()).to_string())
            .collect::<Vec<String>>();
        let key_uid = format!("tss_session{}", self.committee.epoch());
        let message = event.digest().0.to_vec();
        info!(
            "Message to sign has length {}: {:?}",
            message.len(),
            &message
        );
        //let sig_uid = format!("{:x?}", &event.digest().0);
        let sig_uid = hex::encode(&event.digest().0);
        //Hash digest into Vec<u8> of 32 length
        SignInit {
            new_sig_uid: sig_uid,
            key_uid,
            party_uids,
            message_to_sign: message,
        }
    }
    #[allow(clippy::mutable_key_type)]
    async fn process_request_event_verify(
        &self,
        request: anemo::Request<RequestVerifyRequest>,
    ) -> DagResult<RequestVerifyResponse> {
        let event = &request.body().event;
        let event_digest = event.digest();
        if let Some(mut stored_event) = self.event_store.read(&event_digest).await? {
            info!("Stored event {:?}", &stored_event);
            for (authoriry, signature) in event.signatures.iter() {
                stored_event.add_signature(authoriry, signature.clone());
            }
            if let Err(e) = self.event_store.write(&stored_event).await {
                error!("Event store error {:?}", e);
            }
            let mut total_stake = 0;
            for (id, _) in stored_event.signatures.iter() {
                if let Some(authority) = self.committee.authority(id) {
                    total_stake += authority.stake();
                }
            }
            if total_stake >= self.committee.quorum_threshold() {
                info!(
                    "Stake: {}/{}",
                    total_stake,
                    self.committee.quorum_threshold()
                );
                //Request tss then create N&B Transaction
                let sign_message = self.create_sign_init(&stored_event);
                //Send message to tss signer
                //Todo: Handle duplicate
                if let Err(e) = self.tx_tss_sign.send(sign_message) {
                    error!("Send sign message with error {:?}", e);
                } else {
                    info!("Send sign message successfully");
                }
            }
        } else {
            info!("Stored event notfound, write peer's event into the storage");
            self.event_store.write(event).await;
        }
        // Get event from event store
        //
        info!("Received request_event_verify {:?}", event);

        Ok(RequestVerifyResponse {
            event: Some(event.clone()),
        })
    }
}
#[anemo::async_trait]
impl ScalarEvent for ScalarEventService {
    async fn request_event_verify(
        &self,
        request: anemo::Request<RequestVerifyRequest>,
    ) -> Result<anemo::Response<RequestVerifyResponse>, anemo::rpc::Status> {
        self.process_request_event_verify(request)
            .await
            .map(anemo::Response::new)
            .map_err(|e| {
                anemo::rpc::Status::new_with_message(
                    match e {
                        // Report unretriable errors as 400 Bad Request.
                        DagError::InvalidSignature
                        | DagError::InvalidEpoch { .. }
                        | DagError::InvalidHeaderDigest
                        | DagError::HeaderHasBadWorkerIds(_)
                        | DagError::HeaderHasInvalidParentRoundNumbers(_)
                        | DagError::HeaderHasDuplicateParentAuthorities(_)
                        | DagError::AlreadyVoted(_, _, _)
                        | DagError::AlreadyVotedNewerHeader(_, _, _)
                        | DagError::HeaderRequiresQuorum(_)
                        | DagError::TooOld(_, _, _) => {
                            anemo::types::response::StatusCode::BadRequest
                        }
                        // All other errors are retriable.
                        _ => anemo::types::response::StatusCode::Unknown,
                    },
                    format!("{e:?}"),
                )
            })
    }

    async fn create_cross_chain_transaction(
        &self,
        request: anemo::Request<CrossChainTransaction>,
    ) -> Result<anemo::Response<()>, anemo::rpc::Status> {
        let message = request.into_body();
        // if let Err(err) = self
        //     .validator
        //     .validate_batch(&message.batch, &self.protocol_config)
        //     .await
        // {
        //     println!("report_batch {:?}", &message.batch);
        //     return Err(anemo::rpc::Status::new_with_message(
        //         StatusCode::BadRequest,
        //         format!("Invalid batch: {err}"),
        //     ));
        // }
        // let digest = message.batch.digest();

        // let mut batch = message.batch.clone();

        // // TODO: Remove once we have upgraded to protocol version 12.
        // if self.protocol_config.narwhal_versioned_metadata() {
        //     // Set received_at timestamp for remote batch.
        //     batch.versioned_metadata_mut().set_received_at(now());
        // }
        // self.store.insert(&digest, &batch).map_err(|e| {
        //     anemo::rpc::Status::internal(format!("failed to write to batch store: {e:?}"))
        // })?;
        // self.client
        //     .report_others_batch(WorkerOthersBatchMessage {
        //         digest,
        //         worker_id: self.id,
        //     })
        //     .await
        //     .map_err(|e| anemo::rpc::Status::internal(e.to_string()))?;
        Ok(anemo::Response::new(()))
    }
}
// #[derive(Clone, Default, Deserialize, Serialize)]
// pub struct MessageHeader {
//     // Primary that created the header. Must be the same primary that broadcasted the header.
//     // Validation is at: https://github.com/MystenLabs/sui/blob/f0b80d9eeef44edd9fbe606cee16717622b68651/narwhal/primary/src/primary.rs#L713-L719
//     // pub author: AuthorityIdentifier,
//     pub epoch: Epoch,
//     pub payload: [u8; 32],
//     //#[serde(skip)]
//     //digests: OnceCell<HeaderDigest>,
// }

// pub struct MessageCertificate {}
