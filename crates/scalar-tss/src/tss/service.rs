use crate::types::{
    message_in, tss_peer_server::TssPeer, MessageIn, TrafficIn, TssAnemoKeygenRequest,
    TssAnemoKeygenResponse, TssAnemoSignRequest, TssAnemoSignResponse,
};
use anemo::{rpc::Status, Response};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

pub struct TssPeerService {
    tx_keygen: UnboundedSender<MessageIn>,
    tx_sign: UnboundedSender<MessageIn>,
}
impl TssPeerService {
    pub fn new(tx_keygen: UnboundedSender<MessageIn>, tx_sign: UnboundedSender<MessageIn>) -> Self {
        Self { tx_keygen, tx_sign }
    }
}
#[anemo::async_trait]
impl TssPeer for TssPeerService {
    async fn abort(
        &self,
        request: anemo::Request<crate::AbortRequest>,
    ) -> Result<Response<crate::AbortResponse>, Status> {
        let _ = request.into_body();
        let msg_in = MessageIn {
            data: Some(message_in::Data::Abort(false)),
        };

        if let Err(e) = self.tx_sign.send(msg_in) {
            warn!("anemo TssSend error {:?}", e);
        }

        let reply = crate::AbortResponse {
            message: "Abort".to_string(),
        };
        Ok(Response::new(reply))
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
}
