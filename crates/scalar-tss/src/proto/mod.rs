// use crate::types::gg20_server::Gg20;
use crate::types::MessageIn;
// use anemo::{rpc::Status, Response};
use tokio::sync::mpsc::UnboundedSender;
// use tokio_stream::wrappers::UnboundedReceiverStream;
// use tracing::{info, warn};

// include!(concat!(env!("OUT_DIR"), "/gg20.Gg20Peer.rs"));

/// Gg20Service
#[derive(Clone)]
pub struct Gg20AnemoService {
    tx_keygen: UnboundedSender<MessageIn>,
    tx_sign: UnboundedSender<MessageIn>,
}

impl Gg20AnemoService {
    pub fn new(tx_keygen: UnboundedSender<MessageIn>, tx_sign: UnboundedSender<MessageIn>) -> Self {
        Self { tx_keygen, tx_sign }
    }
}

// #[anemo::async_trait]
// impl Gg20 for Gg20AnemoService {
//     type KeygenStream = UnboundedReceiverStream<Result<MessageOut, tonic::Status>>;
//     type SignStream = Self::KeygenStream;

//     async fn keygen(
//         &self,
//         request: anemo::Request<TssAnemoKeygenRequest>,
//     ) -> Result<Response<TssAnemoKeygenResponse>, Status> {
//         info!("Received keygen request");
//         let TssAnemoKeygenRequest { message } = request.into_body();
//         let msg_in = MessageIn {
//             data: Some(message_in::Data::Traffic(TrafficIn {
//                 from_party_uid: message.from_party_uid.clone(),
//                 is_broadcast: message.is_broadcast,
//                 payload: message.payload.clone(),
//             })),
//         };
//         //info!("Received keygen request: {:?}", &body);
//         if let Err(e) = self.tx_keygen.send(msg_in) {
//             warn!("gRpc TssSend error {:?}", e);
//         }

//         let reply = TssAnemoKeygenResponse {
//             message: format!("Process keygen message from {}!", message.from_party_uid),
//         };
//         Ok(Response::new(reply))
//     }
//     async fn sign(
//         &self,
//         request: anemo::Request<TssAnemoSignRequest>,
//     ) -> Result<Response<TssAnemoSignResponse>, Status> {
//         let response = TssAnemoSignResponse {
//             message: "Process sign message!".into(),
//         };
//         Ok(Response::new(response))
//     }
//     async fn recover(
//         &self,
//         request: anemo::Request<RecoverRequest>,
//     ) -> Result<Response<RecoverResponse>, Status> {
//         let response = RecoverResponse { response: 1 };
//         Ok(Response::new(response))
//     }
//     async fn key_presence(
//         &self,
//         request: anemo::Request<KeyPresenceRequest>,
//     ) -> Result<Response<KeyPresenceResponse>, Status> {
//         let response = KeyPresenceResponse { response: 1 };
//         Ok(Response::new(response))
//     }
// }
