//! Handles the keygen streaming gRPC for one party.
//!
//! Protocol:
//!   1. [self::init] First, the initialization message [narwhal_types::KeygenInit] is received from the client.
//!      This message describes the execution of the protocol (i.e. number of participants, share counts, etc).
//!   2. [self::execute] Then, the party starts to generate messages by invoking calls of the [tofn] library until the protocol is completed.
//!      These messages are send to the client using the gRPC stream, and are broadcasted to all participating parties by the client.
//!   3. [self::result] Finally, the party receives the result of the protocol, which is also send to the client through the gRPC stream. Afterwards, the stream is closed.
//!
//! Shares:
//!   Each party might have multiple shares. A single thread is created for each share.
//!   We keep this information agnostic to the client, and we use the [crate::gg20::routing] layer to distribute the messages to each share.
//!   The result of the protocol is common across all shares, and unique for each party. We make use of [self::result] layer to aggregate and process the result.
//!
//! All relevant helper structs and types are defined in [self::types]

use crate::narwhal_types;

use super::{
    broadcast::broadcast_messages, broadcast::broadcast_messages_channel, service::Gg20Service,
    types::ProtocolCommunication,
};

use tonic::Status;

use tofn::{
    collections::TypedUsize,
    gg20::keygen::{
        create_party_keypair_and_zksetup, create_party_keypair_and_zksetup_unsafe, KeygenPartyId,
    },
};

// tonic cruft
use tokio::sync::{mpsc, oneshot};

// logging
use tracing::{info, span, Level, Span};

// error handling
use anyhow::anyhow;
use types::{KeygenInit, MessageIn, MessageOut};
mod execute;
pub mod types;
pub use self::types::*;
mod init;
mod result;
impl Gg20Service {
    pub async fn keygen_init(
        &self,
        keygen_init: KeygenInit,
        mut rx_message_in: mpsc::UnboundedReceiver<MessageIn>,
        mut tx_message_out: mpsc::UnboundedSender<Result<narwhal_types::MessageOut, Status>>,
    ) -> anyhow::Result<()> {
        let keygen_span = span!(Level::INFO, "Keygen");
        // 1. Receive KeygenInit, open message, sanitize arguments -> init mod
        // 2. Spawn N keygen threads to execute the protocol in parallel; one of each of our shares -> execute mod
        // 3. Spawn 1 router thread to route messages from client to the respective keygen thread -> routing mod
        // 4. Wait for all keygen threads to finish and aggregate all responses -> result mod
        // 1. Receive KeygenInit, open message, sanitize arguments -> init mod
        let (keygen_init, key_reservation) = self.process_keygen_init(keygen_init).await?;

        // log keygen init state
        keygen_init.log_info(keygen_span.clone());

        // 2. Spawn N keygen threads to execute the protocol in parallel; one of each of our shares -> execute mod
        let my_share_count = keygen_init.my_shares_count();
        if my_share_count == 0 {
            return Err(anyhow!(
                "Party {} has 0 shares assigned",
                keygen_init.my_index
            ));
        }

        // create in and out channels for each share, and spawn as many threads
        let mut keygen_senders = Vec::with_capacity(my_share_count);
        let mut aggregator_receivers = Vec::with_capacity(my_share_count);

        // computation of (party_keypair, party_zksetup) is intensive so we compute them here once
        let secret_recovery_key = self.kv().seed().await?;
        let session_nonce = keygen_init.new_key_uid.as_bytes();

        info!("Generating keypair for party {} ...", keygen_init.my_index);

        let party_id = TypedUsize::<KeygenPartyId>::from_usize(keygen_init.my_index);

        let party_keygen_data = match self.kv().is_safe_keygen() {
            true => create_party_keypair_and_zksetup(party_id, &secret_recovery_key, session_nonce),
            false => create_party_keypair_and_zksetup_unsafe(
                party_id,
                &secret_recovery_key,
                session_nonce,
            ),
        }
        .map_err(|_| anyhow!("Party keypair generation failed"))?;

        info!(
            "Finished generating keypair for party {}",
            keygen_init.my_index
        );

        for my_tofnd_subindex in 0..my_share_count {
            // channels for communication between router (sender) and protocol threads (receivers)
            let (keygen_sender, keygen_receiver) = mpsc::unbounded_channel();
            keygen_senders.push(keygen_sender);
            // channels for communication between protocol threads (senders) and final result aggregator (receiver)
            let (aggregator_sender, aggregator_receiver) = oneshot::channel();
            aggregator_receivers.push(aggregator_receiver);

            // wrap channels needed by internal threads; receiver chan for router and sender chan gRPC stream
            let chans = ProtocolCommunication::new(keygen_receiver, tx_message_out.clone());
            // wrap all context data needed for each thread
            let ctx = Context::new(
                &keygen_init,
                keygen_init.my_index,
                my_tofnd_subindex,
                party_keygen_data.clone(),
            );
            // clone gg20 service because tokio thread takes ownership
            let gg20 = self.clone();

            // set up log state
            let log_info = ctx.log_info();
            let state = log_info.as_str();
            let execute_span = span!(parent: &keygen_span, Level::DEBUG, "execute", state);

            // spawn keygen thread and continue immediately
            tokio::spawn(async move {
                // wait for keygen's result inside thread
                let secret_key_share = gg20.execute_keygen(chans, &ctx, execute_span).await;
                // send result to aggregator
                info!(
                    "Send secret key share to aggregator {:?}",
                    &secret_key_share
                );
                let _ = aggregator_sender.send(secret_key_share);
            });
        }
        // 3. Spawn 1 router thread to route messages from client to the respective keygen thread -> routing mod
        tokio::spawn(async move {
            broadcast_messages_channel(&mut rx_message_in, keygen_senders, keygen_span).await;
        });

        // 4. Wait for all keygen threads to finish and aggregate all responses -> result mod
        self.aggregate_results(
            aggregator_receivers,
            &mut tx_message_out,
            key_reservation,
            keygen_init,
        )
        .await?;
        Ok(())
    }
}
impl Gg20Service {
    /// handle keygen gRPC
    pub async fn handle_keygen(
        &self,
        mut stream_in: tonic::Streaming<narwhal_types::MessageIn>,
        mut stream_out_sender: mpsc::UnboundedSender<Result<narwhal_types::MessageOut, Status>>,
        keygen_span: Span,
    ) -> anyhow::Result<()> {
        // 1. Receive KeygenInit, open message, sanitize arguments -> init mod
        // 2. Spawn N keygen threads to execute the protocol in parallel; one of each of our shares -> execute mod
        // 3. Spawn 1 router thread to route messages from client to the respective keygen thread -> routing mod
        // 4. Wait for all keygen threads to finish and aggregate all responses -> result mod

        // 1.
        // get KeygenInit message from stream, sanitize arguments and reserve key
        let (keygen_init, key_uid_reservation) = self
            .handle_keygen_init(&mut stream_in, keygen_span.clone())
            .await?;

        // 2.
        // find my share count to allocate channel vectors
        let my_share_count = keygen_init.my_shares_count();
        if my_share_count == 0 {
            return Err(anyhow!(
                "Party {} has 0 shares assigned",
                keygen_init.my_index
            ));
        }

        // create in and out channels for each share, and spawn as many threads
        let mut keygen_senders = Vec::with_capacity(my_share_count);
        let mut aggregator_receivers = Vec::with_capacity(my_share_count);

        // computation of (party_keypair, party_zksetup) is intensive so we compute them here once
        let secret_recovery_key = self.kv().seed().await?;
        let session_nonce = keygen_init.new_key_uid.as_bytes();

        info!("Generating keypair for party {} ...", keygen_init.my_index);

        let party_id = TypedUsize::<KeygenPartyId>::from_usize(keygen_init.my_index);

        let party_keygen_data = match self.kv().is_safe_keygen() {
            true => create_party_keypair_and_zksetup(party_id, &secret_recovery_key, session_nonce),
            false => create_party_keypair_and_zksetup_unsafe(
                party_id,
                &secret_recovery_key,
                session_nonce,
            ),
        }
        .map_err(|_| anyhow!("Party keypair generation failed"))?;

        info!(
            "Finished generating keypair for party {}",
            keygen_init.my_index
        );

        for my_tofnd_subindex in 0..my_share_count {
            // channels for communication between router (sender) and protocol threads (receivers)
            let (keygen_sender, keygen_receiver) = mpsc::unbounded_channel();
            keygen_senders.push(keygen_sender);
            // channels for communication between protocol threads (senders) and final result aggregator (receiver)
            let (aggregator_sender, aggregator_receiver) = oneshot::channel();
            aggregator_receivers.push(aggregator_receiver);

            // wrap channels needed by internal threads; receiver chan for router and sender chan gRPC stream
            let chans = ProtocolCommunication::new(keygen_receiver, stream_out_sender.clone());
            // wrap all context data needed for each thread
            let ctx = Context::new(
                &keygen_init,
                keygen_init.my_index,
                my_tofnd_subindex,
                party_keygen_data.clone(),
            );
            // clone gg20 service because tokio thread takes ownership
            let gg20 = self.clone();

            // set up log state
            let log_info = ctx.log_info();
            let state = log_info.as_str();
            let execute_span = span!(parent: &keygen_span, Level::DEBUG, "execute", state);

            // spawn keygen thread and continue immediately
            tokio::spawn(async move {
                // wait for keygen's result inside thread
                let secret_key_share = gg20.execute_keygen(chans, &ctx, execute_span).await;
                // send result to aggregator
                let _ = aggregator_sender.send(secret_key_share);
            });
        }

        // 3.
        // spin up broadcaster thread and return immediately
        tokio::spawn(async move {
            broadcast_messages(&mut stream_in, keygen_senders, keygen_span).await;
        });

        // 4.
        // wait for all keygen threads to end, aggregate their responses, and store data in KV store
        self.aggregate_results(
            aggregator_receivers,
            &mut stream_out_sender,
            key_uid_reservation,
            keygen_init,
        )
        .await?;

        Ok(())
    }
}
