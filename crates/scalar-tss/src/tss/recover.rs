use tracing::{error, info, warn};

use crate::{recover_response, service::Gg20Service, KeygenInit, KeygenOutput, RecoverRequest};

#[derive(Clone)]
pub struct TssRecover {
    pub gg20_service: Gg20Service,
}

impl TssRecover {
    pub fn new(gg20_service: Gg20Service) -> Self {
        Self { gg20_service }
    }

    pub async fn execute_recover(&self, keygen_init: KeygenInit, keygen_output: KeygenOutput) {
        let recover_request = RecoverRequest {
            keygen_init: Some(keygen_init),
            keygen_output: Some(keygen_output),
        };
        let response = self.gg20_service.handle_recover(recover_request).await;

        let response = match response {
            Ok(()) => {
                info!("Recovery completed successfully!");
                recover_response::Response::Success
            }
            Err(err) => {
                error!("Unable to complete recovery: {}", err);
                recover_response::Response::Fail
            }
        };

        // prost way to convert i32 to enums https://github.com/danburkert/prost#enumerations
        match recover_response::Response::from_i32(response as i32) {
            Some(recover_response::Response::Success) => {
                info!("Got success from recover")
            }
            Some(recover_response::Response::Fail) => {
                warn!("Got fail from recover")
            }
            Some(recover_response::Response::Unspecified) => {
                panic!("Unspecified recovery response. Expecting Success/Fail")
            }
            None => {
                panic!("Invalid recovery response. Could not convert i32 to enum")
            }
        }
    }
}
