//! This module handles the key_presence gRPC.
//! Request includes [narwhal_types::message_in::Data::KeyPresenceRequest] struct and encrypted recovery info.

use super::service::MultisigService;

// logging
use tracing::debug;

// error handling

impl MultisigService {
    pub(super) async fn handle_key_presence(
        &self,
        request: narwhal_types::KeyPresenceRequest,
    ) -> anyhow::Result<narwhal_types::key_presence_response::Response> {
        // check if mnemonic is available
        let _ = self
            .find_matching_seed(&request.key_uid, &request.pub_key)
            .await?;

        // key presence for multisig always returns `Present`.
        // this is done in order to not break compatibility with axelar-core
        // TODO: better handling for multisig key presence.
        debug!(
            "[{}] key presence check for multisig always return Present",
            request.key_uid
        );
        Ok(narwhal_types::key_presence_response::Response::Present)
    }
}
