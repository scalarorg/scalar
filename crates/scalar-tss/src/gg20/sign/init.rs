//! This module handles the initialization of the Sign protocol.
//! A [SignInitSanitized] struct is created out of the raw incoming [narwhal_types::SignInit] message and the session key is queried inside from KvStore.
//! If [narwhal_types::SignInit] fails to be parsed, or no Keygen has been executed for the current session ID, an [anyhow!] error is returned

// try_into() for MessageDigest
use std::array::TryFromSliceError;

use super::{types::SignInitSanitized, Gg20Service};
use crate::types::{message_in, MessageIn, MessageOut, SignInit};
use crate::{
    gg20::types::{MessageDigest, PartyInfo},
    TofndResult,
};

// tonic cruft
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tonic::Status;

// logging
use tracing::{debug, Span};

// error handling
use anyhow::anyhow;

impl Gg20Service {
    /// Receives a message from the stream and tries to handle sign init operations.
    /// On success, it extracts the PartyInfo from the KVStrore and returns a sanitized struct ready to be used by the protocol.
    /// On failure, returns an [anyhow!] error and no changes are been made in the KvStore.
    pub(super) async fn handle_sign_init(
        &self,
        in_stream: &mut tonic::Streaming<MessageIn>,
        out_stream: &mut mpsc::UnboundedSender<Result<MessageOut, Status>>,
        sign_span: Span,
    ) -> TofndResult<(SignInitSanitized, PartyInfo)> {
        let msg_type = in_stream
            .next()
            .await
            .ok_or_else(|| anyhow!("sign: stream closed by client without sending a message"))??
            .data
            .ok_or_else(|| anyhow!("sign: missing `data` field in client message"))?;
        let sign_init = match msg_type {
            message_in::Data::SignInit(k) => k,
            _ => return Err(anyhow!("Expected sign init message")),
        };
        debug!("Hanle sign init key uid {:?}", &sign_init.key_uid);
        // try to get party info related to session id
        let party_info: PartyInfo = match self.get_party_info(&sign_init.key_uid).await {
            Ok(value) => value,
            Err(err) => {
                Self::send_kv_store_failure(out_stream)?;
                let err = anyhow!("Unable to find session-id {} in kv store. Issuing share recovery and exit sign {:?}", sign_init.key_uid, err);
                return Err(err);
            }
        };
        // let party_info: PartyInfo = match self.kv_manager.kv().get(&sign_init.key_uid).await {
        //     Ok(value) => value.try_into()?,
        //     Err(err) => {
        //         // if no such session id exists, send a message to client that indicates that recovery is needed and stop sign
        //         Self::send_kv_store_failure(out_stream)?;
        //         let err = anyhow!("Unable to find session-id {} in kv store. Issuing share recovery and exit sign {:?}", sign_init.key_uid, err);
        //         return Err(err);
        //     }
        // };
        debug!("try to sanitize arguments");
        // try to sanitize arguments
        let sign_init = Self::sign_sanitize_args(sign_init, &party_info.tofnd.party_uids)?;

        // log SignInitSanitized state
        party_info.log_info(&sign_init.new_sig_uid, sign_span);
        debug!("finish sign init");
        Ok((sign_init, party_info))
    }

    /// send "need recover" message to client
    fn send_kv_store_failure(
        out_stream: &mut mpsc::UnboundedSender<Result<MessageOut, Status>>,
    ) -> TofndResult<()> {
        Ok(out_stream.send(Ok(MessageOut::need_recover()))?)
    }

    /// sanitize arguments of incoming message.
    /// Example:
    /// input for party 'a':
    ///   (from keygen) party_uids = [a, b, c]
    ///   (from keygen) party_share_counts = [3, 2, 1]
    ///   narwhal_types::SignInit.party_uids = [c, a]
    /// output for party 'a':
    ///   SignInitSanitized.party_uids = [2, 0]  <- index of c, a in party_uids
    fn sign_sanitize_args(
        sign_init: SignInit,
        all_party_uids: &[String],
    ) -> TofndResult<SignInitSanitized> {
        // create a vector of the tofnd indices of the participant uids
        let participant_indices = sign_init
            .party_uids
            .iter()
            .map(|s| {
                all_party_uids.iter().position(|k| k == s).ok_or_else(|| {
                    anyhow!(
                        "participant [{}] not found in key [{}]",
                        s,
                        sign_init.key_uid
                    )
                })
            })
            .collect::<Result<Vec<usize>, _>>()?;
        debug!("Participant indices {:?}", &participant_indices);
        let message = sign_init.message_to_sign.clone();
        debug!("Message to sign {:?}", &message);
        let digest: Result<MessageDigest, TryFromSliceError> = message.as_slice().try_into();
        debug!("{:?}", &digest);
        Ok(SignInitSanitized {
            new_sig_uid: sign_init.new_sig_uid,
            participant_uids: sign_init.party_uids,
            participant_indices,
            message_to_sign: message.as_slice().try_into()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_sign_sanitize_args() {
        let all_party_uids = vec![
            "party_0".to_owned(), // party 0 has index 0
            "party_1".to_owned(), // party 1 has index 1
            "party_2".to_owned(), // party 2 has index 2
        ];

        let raw_sign_init = SignInit {
            new_sig_uid: "test_uid".to_owned(),
            key_uid: "test_uid".to_owned(),
            party_uids: vec!["party_2".to_owned(), "party_1".to_owned()],
            message_to_sign: vec![42; 32],
        };
        let sanitized_sign_init = SignInitSanitized {
            new_sig_uid: "test_uid".to_owned(), // new sig uid should be the same
            participant_uids: vec!["party_2".to_owned(), "party_1".to_owned()], // party 2 has index 2, party 1 has index 1
            participant_indices: vec![2, 1], // indices should be [2, 1]
            message_to_sign: vec![42; 32].as_slice().try_into().unwrap(), // msg of 32 bytes should be successfully converted to MessageDigest
        };

        let res = Gg20Service::sign_sanitize_args(raw_sign_init, &all_party_uids).unwrap();
        assert_eq!(&res.new_sig_uid, &sanitized_sign_init.new_sig_uid);
        assert_eq!(&res.participant_uids, &sanitized_sign_init.participant_uids);
        assert_eq!(
            &res.participant_indices,
            &sanitized_sign_init.participant_indices
        );
        assert_eq!(&res.message_to_sign, &sanitized_sign_init.message_to_sign);
    }

    #[test]
    fn test_fail_sign_sanitize_args() {
        let all_party_uids = vec![
            "party_0".to_owned(),
            "party_1".to_owned(),
            "party_2".to_owned(),
        ];
        let raw_sign_init = SignInit {
            new_sig_uid: "test_uid".to_owned(),
            key_uid: "test_uid".to_owned(),
            party_uids: vec!["party_4".to_owned(), "party_1".to_owned()], // party 4 does not exist
            message_to_sign: vec![42; 32],
        };
        assert!(Gg20Service::sign_sanitize_args(raw_sign_init, &all_party_uids).is_err());

        let raw_sign_init = SignInit {
            new_sig_uid: "test_uid".to_owned(),
            key_uid: "test_uid".to_owned(),
            party_uids: vec!["party_2".to_owned(), "party_1".to_owned()],
            message_to_sign: vec![42; 33], // message is not 32 bytes
        };
        assert!(Gg20Service::sign_sanitize_args(raw_sign_init, &all_party_uids).is_err());
    }
}
