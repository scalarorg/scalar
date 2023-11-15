// ! This module handles the key_presence anemo.
// ! Request includes [proto::message_in::Data::KeyPresenceRequest] struct and encrypted recovery info.
// ! The recovery info is decrypted by party's mnemonic seed and saved in the KvStore.

use super::service::Gg20Service;
use crate::types::{key_presence_response, KeyPresenceRequest};

// logging
use tracing::info;

impl Gg20Service {
    pub async fn handle_key_presence(
        &self,
        request: KeyPresenceRequest,
    ) -> anyhow::Result<key_presence_response::Response> {
        // check if mnemonic is available
        let _ = self.kv_store.seed().await?;

        // check if requested key exists
        if self.kv_store.tss_store().exists(&request.key_uid).await? {
            info!(
                "Found session-id {} in kv store during key presence check",
                request.key_uid
            );
            Ok(key_presence_response::Response::Present)
        } else {
            info!(
                "Did not find session-id {} in kv store during key presence check",
                request.key_uid
            );
            Ok(key_presence_response::Response::Absent)
        }
    }
}
