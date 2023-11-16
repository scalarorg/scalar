use tracing::{error, info};

use crate::{key_presence_response, service::Gg20Service, KeyPresenceRequest};

#[derive(Clone)]
pub struct TssKeyPresence {
    pub gg20_service: Gg20Service,
}

impl TssKeyPresence {
    pub fn new(gg20_service: Gg20Service) -> Self {
        Self { gg20_service }
    }

    pub async fn execute_key_presence(&self, key_uid: String) -> bool {
        let key_presence_request = KeyPresenceRequest {
            key_uid,
            pub_key: vec![],
        };

        let response = match self
            .gg20_service
            .handle_key_presence(key_presence_request)
            .await
        {
            Ok(res) => {
                info!("Key presence check completed succesfully!");
                res
            }
            Err(err) => {
                error!("Unable to complete key presence check: {}", err);
                key_presence_response::Response::Fail
            }
        };

        // prost way to convert i32 to enums https://github.com/danburkert/prost#enumerations
        match key_presence_response::Response::from_i32(response as i32) {
            Some(key_presence_response::Response::Present) => true,
            Some(key_presence_response::Response::Absent) => false,
            Some(key_presence_response::Response::Fail) => {
                panic!("key presence request failed")
            }
            Some(key_presence_response::Response::Unspecified) => {
                panic!("Unspecified key presence response")
            }
            None => {
                panic!("Invalid key presence response. Could not convert i32 to enum")
            }
        }
    }
}
