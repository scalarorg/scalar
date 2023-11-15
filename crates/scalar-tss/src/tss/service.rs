use crate::{
    service::Gg20Service,
    types::{
        key_presence_response, message_in, recover_response, tss_peer_server::TssPeer,
        KeyPresenceRequest, KeyPresenceResponse, MessageIn, RecoverRequest, RecoverResponse,
        TrafficIn, TssAnemoKeygenRequest, TssAnemoKeygenResponse, TssAnemoSignRequest,
        TssAnemoSignResponse, TssAnemoVerifyRequest, TssAnemoVerifyResponse,
    },
};
use anemo::{rpc::Status, Response};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info, warn};

pub struct TssPeerService {
    tx_keygen: UnboundedSender<MessageIn>,
    tx_sign: UnboundedSender<MessageIn>,
    gg20_service: Gg20Service,
}
impl TssPeerService {
    pub fn new(
        tx_keygen: UnboundedSender<MessageIn>,
        tx_sign: UnboundedSender<MessageIn>,
        gg20_service: Gg20Service,
    ) -> Self {
        Self {
            tx_keygen,
            tx_sign,
            gg20_service,
        }
    }
}
#[anemo::async_trait]
impl TssPeer for TssPeerService {
    /// Recover. See [recover].
    async fn recover(
        &self,
        request: anemo::Request<RecoverRequest>,
    ) -> Result<Response<RecoverResponse>, Status> {
        let request = request.into_body();

        let response = self.gg20_service.handle_recover(request).await;
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

        Ok(Response::new(RecoverResponse {
            // the prost way to convert enums to i32 https://github.com/danburkert/prost#enumerations
            response: response as i32,
        }))
    }

    /// KeyPresence. See [key_presence].
    async fn key_presence(
        &self,
        request: anemo::Request<KeyPresenceRequest>,
    ) -> Result<Response<KeyPresenceResponse>, Status> {
        let request = request.into_body();

        let response = match self.gg20_service.handle_key_presence(request).await {
            Ok(res) => {
                info!("Key presence check completed succesfully!");
                res
            }
            Err(err) => {
                error!("Unable to complete key presence check: {}", err);
                key_presence_response::Response::Fail
            }
        };

        Ok(Response::new(KeyPresenceResponse {
            response: response as i32,
        }))
    }

    async fn keygen(
        &self,
        request: anemo::Request<TssAnemoKeygenRequest>,
    ) -> Result<Response<TssAnemoKeygenResponse>, Status> {
        info!("Received keygen request");
        let TssAnemoKeygenRequest { message } = request.into_body();
        let msg_in = MessageIn {
            data: Some(message_in::Data::Traffic(TrafficIn {
                from_party_uid: message.from_party_uid.clone(),
                is_broadcast: message.is_broadcast,
                payload: message.payload.clone(),
            })),
        };
        //info!("Received keygen request: {:?}", &body);
        info!("Received keygen request");
        if let Err(e) = self.tx_keygen.send(msg_in) {
            warn!("gRpc TssSend error {:?}", e);
        }

        let reply = TssAnemoKeygenResponse {
            message: format!("Process keygen message from {}!", message.from_party_uid),
        };
        Ok(Response::new(reply))
    }
    async fn sign(
        &self,
        request: anemo::Request<TssAnemoSignRequest>,
    ) -> Result<Response<TssAnemoSignResponse>, Status> {
        info!("Received sign request");
        let TssAnemoSignRequest { message } = request.into_body();
        let msg_in = MessageIn {
            data: Some(message_in::Data::Traffic(TrafficIn {
                from_party_uid: message.from_party_uid.clone(),
                is_broadcast: message.is_broadcast,
                payload: message.payload.clone(),
            })),
        };
        if let Err(e) = self.tx_sign.send(msg_in) {
            warn!("gRpc TssSend error {:?}", e);
        }
        let reply = TssAnemoSignResponse {
            message: format!(
                "Send sign broadcast message from {}!",
                message.from_party_uid
            ),
        };
        Ok(Response::new(reply))
    }
    async fn verify(
        &self,
        request: anemo::Request<TssAnemoVerifyRequest>,
    ) -> Result<Response<TssAnemoVerifyResponse>, Status> {
        let TssAnemoVerifyRequest { message } = request.into_body();
        // let msg_in = MessageIn {
        //     data: Some(message_in::Data::Traffic(TrafficIn {
        //         from_party_uid: message.from_party_uid.clone(),
        //         is_broadcast: message.is_broadcast,
        //         payload: message.payload.clone(),
        //     })),
        // };
        let reply = TssAnemoVerifyResponse {
            message: format!("Hello {}!", message.from_party_uid),
        };
        Ok(Response::new(reply))
    }
}
