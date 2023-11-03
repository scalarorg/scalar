//! This module handles the routing of incoming traffic.
//! Receives and validates messages until the connection is closed by the client.
//! The incoming messages come from the gRPC stream and are forwarded to shares' internal channels.

// tonic cruft
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tonic::Status;

// logging
use tracing::{debug, info, span, warn, Level, Span};

use crate::tss::narwhal_types;

/// Results of routing
#[derive(Debug, PartialEq)]
enum RoutingStatus {
    Continue { traffic: narwhal_types::TrafficIn },
    Stop,
    Skip,
}

/// Receives incoming from a gRPC stream and broadcasts them to internal channels;
/// Loops until client closes the socket, or a message containing [narwhal_types::message_in::Data::Abort] is received  
/// Empty and unknown messages are ignored
pub(super) async fn broadcast_messages_channel(
    rx_message_in: &mut mpsc::UnboundedReceiver<narwhal_types::MessageIn>,
    mut out_internal_channels: Vec<mpsc::UnboundedSender<Option<narwhal_types::TrafficIn>>>,
    span: Span,
) {
    // loop until `stop` is received
    loop {
        // read message from stream
        if let Some(msg_data) = rx_message_in.recv().await {
            info!("Received message from anemo service");
            // check incoming message
            let traffic = match open_message_in(msg_data, span.clone()) {
                RoutingStatus::Continue { traffic } => traffic,
                RoutingStatus::Stop => break,
                RoutingStatus::Skip => continue,
            };

            // send the message to all channels
            for out_channel in &mut out_internal_channels {
                info!("Send receiver message to internal_channels");
                let _ = out_channel.send(Some(traffic.clone()));
            }
        }
    }
}

fn open_message_in(msg: narwhal_types::MessageIn, span: Span) -> RoutingStatus {
    // start routing span
    let route_span = span!(parent: &span, Level::INFO, "routing");
    let _start = route_span.enter();

    // get data option

    // TODO examine why this happens: Sometimes, when the connection is
    // closed by the client, instead of a `None` message, we get a `Some`
    // message containing an "error reading a body from connection: protocol
    // error: not a result of an error" error Removing for now to prevent this
    // message from appearing while doing keygen/signs but will need to
    // find out why this happens
    // https://github.com/axelarnetwork/tofnd/issues/167

    // get message data
    let msg_data = match msg.data {
        Some(data) => data,
        None => {
            warn!("ignore incoming msg: missing `data` field");
            return RoutingStatus::Skip;
        }
    };

    // match message data to types
    let traffic = match msg_data {
        narwhal_types::message_in::Data::Traffic(t) => t,
        narwhal_types::message_in::Data::Abort(_) => {
            warn!("received abort message");
            return RoutingStatus::Stop;
        }
        narwhal_types::message_in::Data::KeygenInit(_)
        | narwhal_types::message_in::Data::SignInit(_) => {
            warn!("ignore incoming msg: expect `data` to be TrafficIn type");
            return RoutingStatus::Skip;
        }
    };

    // return traffic
    RoutingStatus::Continue { traffic }
}

/// Receives incoming from a gRPC stream and broadcasts them to internal channels;
/// Loops until client closes the socket, or a message containing [narwhal_types::message_in::Data::Abort] is received  
/// Empty and unknown messages are ignored
pub(super) async fn broadcast_messages(
    in_grpc_stream: &mut tonic::Streaming<narwhal_types::MessageIn>,
    mut out_internal_channels: Vec<mpsc::UnboundedSender<Option<narwhal_types::TrafficIn>>>,
    span: Span,
) {
    // loop until `stop` is received
    loop {
        // read message from stream
        let msg_data = in_grpc_stream.next().await;
        info!("Received message from grpc_stream");
        // check incoming message
        let traffic = match open_message(msg_data, span.clone()) {
            RoutingStatus::Continue { traffic } => traffic,
            RoutingStatus::Stop => break,
            RoutingStatus::Skip => continue,
        };

        // send the message to all channels
        for out_channel in &mut out_internal_channels {
            let _ = out_channel.send(Some(traffic.clone()));
        }
    }
}

/// gets a gPRC [narwhal_types::MessageIn] and checks the type
/// available messages are:
/// [narwhal_types::message_in::Data::Traffic]    -> return [RoutingResult::Continue]
/// [narwhal_types::message_in::Data::Abort]      -> return [RoutingResult::Stop]
/// [narwhal_types::message_in::Data::KeygenInit] -> return [RoutingResult::Skip]
/// [narwhal_types::message_in::Data::SignInit]   -> return [RoutingResult::Skip]
fn open_message(
    msg: Option<Result<narwhal_types::MessageIn, Status>>,
    span: Span,
) -> RoutingStatus {
    // start routing span
    let route_span = span!(parent: &span, Level::INFO, "routing");
    let _start = route_span.enter();

    // we receive MessageIn wrapped in multiple layers. We have to unpeel tonic message

    // get result
    let msg_result = match msg {
        Some(msg_result) => msg_result,
        None => {
            info!("Stream closed");
            return RoutingStatus::Stop;
        }
    };

    // get data option

    // TODO examine why this happens: Sometimes, when the connection is
    // closed by the client, instead of a `None` message, we get a `Some`
    // message containing an "error reading a body from connection: protocol
    // error: not a result of an error" error Removing for now to prevent this
    // message from appearing while doing keygen/signs but will need to
    // find out why this happens
    // https://github.com/axelarnetwork/tofnd/issues/167
    let msg_data_opt = match msg_result {
        Ok(msg_in) => msg_in.data,
        Err(err) => {
            info!("Stream closed");
            debug!("Stream closed with err {}", err);
            return RoutingStatus::Stop;
        }
    };

    // get message data
    let msg_data = match msg_data_opt {
        Some(msg_data) => msg_data,
        None => {
            warn!("ignore incoming msg: missing `data` field");
            return RoutingStatus::Skip;
        }
    };

    // match message data to types
    let traffic = match msg_data {
        narwhal_types::message_in::Data::Traffic(t) => t,
        narwhal_types::message_in::Data::Abort(_) => {
            warn!("received abort message");
            return RoutingStatus::Stop;
        }
        narwhal_types::message_in::Data::KeygenInit(_)
        | narwhal_types::message_in::Data::SignInit(_) => {
            warn!("ignore incoming msg: expect `data` to be TrafficIn type");
            return RoutingStatus::Skip;
        }
    };

    // return traffic
    RoutingStatus::Continue { traffic }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCase {
        message_in: narwhal_types::MessageIn,
        expected_result: RoutingStatus,
    }

    impl TestCase {
        fn new(message_in: narwhal_types::MessageIn, expected_result: RoutingStatus) -> Self {
            TestCase {
                message_in,
                expected_result,
            }
        }
    }

    fn new_msg_in(msg_in: narwhal_types::message_in::Data) -> narwhal_types::MessageIn {
        narwhal_types::MessageIn { data: Some(msg_in) }
    }

    #[test]
    fn test_validate_message() {
        let test_cases = vec![
            TestCase::new(
                new_msg_in(narwhal_types::message_in::Data::Abort(true)),
                RoutingStatus::Stop,
            ),
            TestCase::new(
                new_msg_in(narwhal_types::message_in::Data::KeygenInit(
                    narwhal_types::KeygenInit::default(),
                )),
                RoutingStatus::Skip,
            ),
            TestCase::new(
                new_msg_in(narwhal_types::message_in::Data::SignInit(
                    narwhal_types::SignInit::default(),
                )),
                RoutingStatus::Skip,
            ),
            TestCase::new(
                new_msg_in(narwhal_types::message_in::Data::Traffic(
                    narwhal_types::TrafficIn::default(),
                )),
                RoutingStatus::Continue {
                    traffic: narwhal_types::TrafficIn::default(),
                },
            ),
            TestCase::new(narwhal_types::MessageIn { data: None }, RoutingStatus::Skip),
        ];

        let span = span!(Level::INFO, "test-span");

        for test_case in test_cases {
            let result = open_message(Some(Ok(test_case.message_in)), span.clone());
            assert_eq!(result, test_case.expected_result);
        }

        let result = open_message(Some(Err(tonic::Status::ok("test status"))), span.clone());
        assert_eq!(result, RoutingStatus::Stop);

        let result = open_message(None, span);
        assert_eq!(result, RoutingStatus::Stop);
    }
}
