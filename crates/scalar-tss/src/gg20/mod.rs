//! [narwhal_types::gg20_server::Gg20] gRPC server API
//! Available gRPCs are:
//!     [recover] - Recovers private data of a party provided a mnemonic.
//!     [keygen] - Starts keygen.
//!     [sign] - Starts sing.
// use super::narwhal_types;

// tonic cruft
// use tokio::sync::mpsc;
// use tokio_stream::wrappers::UnboundedReceiverStream;
// use tonic::{Request, Response, Status};

// logging
// use crate::types::gg20::{
//     KeyPresenceRequest, KeyPresenceResponse, KeygenRequest, KeygenResponse, RecoverRequest,
//     RecoverResponse, SignRequest, SignResponse,
// };
// use tracing::{error, info, span, Level};

// gRPC
mod broadcast;
mod key_presence;
mod keygen;
mod protocol;
mod recover;
pub mod service;
mod sign;
pub mod types;
use self::types::*;

// #[tonic::async_trait]
// impl narwhal_types::gg20_server::Gg20 for service::Gg20Service {
//     type KeygenStream = UnboundedReceiverStream<Result<narwhal_types::MessageOut, tonic::Status>>;
//     type SignStream = Self::KeygenStream;

//     /// Recover unary gRPC. See [recover].
//     async fn recover(
//         &self,
//         request: tonic::Request<narwhal_types::RecoverRequest>,
//     ) -> Result<Response<narwhal_types::RecoverResponse>, Status> {
//         let request = request.into_inner();

//         let response = self.handle_recover(request).await;
//         let response = match response {
//             Ok(()) => {
//                 info!("Recovery completed successfully!");
//                 narwhal_types::recover_response::Response::Success
//             }
//             Err(err) => {
//                 error!("Unable to complete recovery: {}", err);
//                 narwhal_types::recover_response::Response::Fail
//             }
//         };

//         Ok(Response::new(narwhal_types::RecoverResponse {
//             // the prost way to convert enums to i32 https://github.com/danburkert/prost#enumerations
//             response: response as i32,
//         }))
//     }

//     /// KeyPresence unary gRPC. See [key_presence].
//     async fn key_presence(
//         &self,
//         request: tonic::Request<narwhal_types::KeyPresenceRequest>,
//     ) -> Result<Response<narwhal_types::KeyPresenceResponse>, Status> {
//         let request = request.into_inner();

//         let response = match self.handle_key_presence(request).await {
//             Ok(res) => {
//                 info!("Key presence check completed succesfully!");
//                 res
//             }
//             Err(err) => {
//                 error!("Unable to complete key presence check: {}", err);
//                 narwhal_types::key_presence_response::Response::Fail
//             }
//         };

//         Ok(Response::new(narwhal_types::KeyPresenceResponse {
//             response: response as i32,
//         }))
//     }

//     /// Keygen streaming gRPC. See [keygen].
//     async fn keygen(
//         &self,
//         request: Request<tonic::Streaming<narwhal_types::MessageIn>>,
//     ) -> Result<Response<Self::KeygenStream>, Status> {
//         let stream_in = request.into_inner();
//         let (msg_sender, rx) = mpsc::unbounded_channel();

//         // log span for keygen
//         let span = span!(Level::INFO, "Keygen");
//         let _enter = span.enter();
//         let s = span.clone();
//         let gg20 = self.clone();

//         tokio::spawn(async move {
//             // can't return an error from a spawned thread
//             if let Err(e) = gg20.handle_keygen(stream_in, msg_sender.clone(), s).await {
//                 error!("keygen failure: {:?}", e.to_string());
//                 // we can't handle errors in tokio threads. Log error if we are unable to send the status code to client.
//                 if let Err(e) = msg_sender.send(Err(Status::invalid_argument(e.to_string()))) {
//                     error!("could not send error to client: {}", e.to_string());
//                 }
//             }
//         });
//         Ok(Response::new(UnboundedReceiverStream::new(rx)))
//     }

//     /// Sign sreaming gRPC. See [sign].
//     async fn sign(
//         &self,
//         request: Request<tonic::Streaming<narwhal_types::MessageIn>>,
//     ) -> Result<Response<Self::SignStream>, Status> {
//         let stream = request.into_inner();
//         let (msg_sender, rx) = mpsc::unbounded_channel();

//         // log span for sign
//         let span = span!(Level::INFO, "Sign");
//         let _enter = span.enter();
//         let s = span.clone();
//         let gg20 = self.clone();

//         tokio::spawn(async move {
//             // can't return an error from a spawned thread
//             if let Err(e) = gg20.handle_sign(stream, msg_sender.clone(), s).await {
//                 error!("sign failure: {:?}", e.to_string());
//                 // we can't handle errors in tokio threads. Log error if we are unable to send the status code to client.
//                 if let Err(e) = msg_sender.send(Err(Status::invalid_argument(e.to_string()))) {
//                     error!("could not send error to client: {}", e.to_string());
//                 }
//             }
//         });
//         Ok(Response::new(UnboundedReceiverStream::new(rx)))
//     }
// }
