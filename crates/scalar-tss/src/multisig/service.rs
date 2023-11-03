use tonic::Response;
use tonic::Status;
include!(concat!(env!("OUT_DIR"), "/multisig.MultisigPeer.rs"));
use crate::kv_manager::KvManager;

use tracing::{error, info};

/// Gg20Service
#[derive(Clone)]
pub struct MultisigService {
    pub(super) kv_manager: KvManager,
}

/// create a new Multisig gRPC server
pub fn new_service(kv_manager: KvManager) -> impl narwhal_types::multisig_server::Multisig {
    MultisigService { kv_manager }
}

#[tonic::async_trait]
impl narwhal_types::multisig_server::Multisig for MultisigService {
    async fn key_presence(
        &self,
        request: tonic::Request<narwhal_types::KeyPresenceRequest>,
    ) -> Result<Response<narwhal_types::KeyPresenceResponse>, Status> {
        let request = request.into_inner();

        let response = match self.handle_key_presence(request).await {
            Ok(res) => {
                info!("Key presence check completed succesfully");
                res
            }
            Err(err) => {
                error!("Unable to complete key presence check: {}", err);
                narwhal_types::key_presence_response::Response::Fail
            }
        };

        Ok(Response::new(narwhal_types::KeyPresenceResponse {
            response: response as i32,
        }))
    }

    async fn keygen(
        &self,
        request: tonic::Request<narwhal_types::KeygenRequest>,
    ) -> Result<Response<narwhal_types::KeygenResponse>, Status> {
        let request = request.into_inner();
        let result = match self.handle_keygen(&request).await {
            Ok(pub_key) => {
                info!(
                    "[{}] Multisig Keygen with key id [{}] completed",
                    request.party_uid, request.key_uid
                );
                narwhal_types::keygen_response::KeygenResponse::PubKey(pub_key)
            }
            Err(err) => {
                error!(
                    "[{}] Multisig Keygen with key id [{}] failed: {}",
                    request.party_uid,
                    request.key_uid,
                    err.to_string()
                );
                narwhal_types::keygen_response::KeygenResponse::Error(err.to_string())
            }
        };

        Ok(Response::new(narwhal_types::KeygenResponse {
            keygen_response: Some(result),
        }))
    }

    async fn sign(
        &self,
        request: tonic::Request<narwhal_types::SignRequest>,
    ) -> Result<Response<narwhal_types::SignResponse>, Status> {
        let request = request.into_inner();
        let result = match self.handle_sign(&request).await {
            Ok(pub_key) => {
                info!(
                    "[{}] Multisig Sign with key id [{}] and message [{:?}] completed",
                    request.party_uid, request.key_uid, request.msg_to_sign,
                );
                narwhal_types::sign_response::SignResponse::Signature(pub_key)
            }
            Err(err) => {
                error!(
                    "[{}] Multisig sign with key id [{}] and message [{:?}] failed: {}",
                    request.party_uid,
                    request.key_uid,
                    request.msg_to_sign,
                    err.to_string()
                );
                narwhal_types::sign_response::SignResponse::Error(err.to_string())
            }
        };

        Ok(Response::new(narwhal_types::SignResponse {
            sign_response: Some(result),
        }))
    }
}
