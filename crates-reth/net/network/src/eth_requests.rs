//! Blocks/Headers management for the p2p network.

use crate::{metrics::EthRequestHandlerMetrics, peers::PeersHandle};
use futures::StreamExt;
use reth_eth_wire::{
    BlockBodies, BlockHeaders, GetBlockBodies, GetBlockHeaders, GetNodeData, GetReceipts, NodeData,
    Receipts,
};
use reth_interfaces::p2p::error::RequestResult;
use reth_primitives::{BlockBody, BlockHashOrNumber, Header, HeadersDirection, PeerId};
use reth_provider::{BlockReader, HeaderProvider, ReceiptProvider};
use std::{
    borrow::Borrow,
    future::Future,
    hash::Hash,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::{mpsc::Receiver, oneshot};
use tokio_stream::wrappers::ReceiverStream;

// Limits: <https://github.com/ethereum/go-ethereum/blob/b0d44338bbcefee044f1f635a84487cbbd8f0538/eth/protocols/eth/handler.go#L34-L56>

/// Maximum number of receipts to serve.
///
/// Used to limit lookups.
const MAX_RECEIPTS_SERVE: usize = 1024;

/// Maximum number of block headers to serve.
///
/// Used to limit lookups.
const MAX_HEADERS_SERVE: usize = 1024;

/// Maximum number of block headers to serve.
///
/// Used to limit lookups. With 24KB block sizes nowadays, the practical limit will always be
/// SOFT_RESPONSE_LIMIT.
const MAX_BODIES_SERVE: usize = 1024;

/// Estimated size in bytes of an RLP encoded receipt.
const APPROX_RECEIPT_SIZE: usize = 24 * 1024;

/// Estimated size in bytes of an RLP encoded body.
// TODO: check 24kb blocksize assumption
const APPROX_BODY_SIZE: usize = 24 * 1024;

/// Maximum size of replies to data retrievals.
const SOFT_RESPONSE_LIMIT: usize = 2 * 1024 * 1024;

/// Estimated size in bytes of an RLP encoded header.
const APPROX_HEADER_SIZE: usize = 500;

/// Manages eth related requests on top of the p2p network.
///
/// This can be spawned to another task and is supposed to be run as background service.
#[derive(Debug)]
#[must_use = "Manager does nothing unless polled."]
pub struct EthRequestHandler<C> {
    /// The client type that can interact with the chain.
    client: C,
    /// Used for reporting peers.
    #[allow(unused)]
    // TODO use to report spammers
    peers: PeersHandle,
    /// Incoming request from the [NetworkManager](crate::NetworkManager).
    incoming_requests: ReceiverStream<IncomingEthRequest>,
    /// Metrics for the eth request handler.
    metrics: EthRequestHandlerMetrics,
}

// === impl EthRequestHandler ===
impl<C> EthRequestHandler<C> {
    /// Create a new instance
    pub fn new(client: C, peers: PeersHandle, incoming: Receiver<IncomingEthRequest>) -> Self {
        let metrics = Default::default();
        Self { client, peers, incoming_requests: ReceiverStream::new(incoming), metrics }
    }
}

impl<C> EthRequestHandler<C>
where
    C: BlockReader + HeaderProvider + ReceiptProvider,
{
    /// Returns the list of requested headers
    fn get_headers_response(&self, request: GetBlockHeaders) -> Vec<Header> {
        let GetBlockHeaders { start_block, limit, skip, direction } = request;

        let mut headers = Vec::new();

        let mut block: BlockHashOrNumber = match start_block {
            BlockHashOrNumber::Hash(start) => start.into(),
            BlockHashOrNumber::Number(num) => {
                let Some(hash) = self.client.block_hash(num).unwrap_or_default() else {
                    return headers
                };
                hash.into()
            }
        };

        let skip = skip as u64;
        let mut total_bytes = APPROX_HEADER_SIZE;

        for _ in 0..limit {
            if let Some(header) = self.client.header_by_hash_or_number(block).unwrap_or_default() {
                match direction {
                    HeadersDirection::Rising => {
                        if let Some(next) = (header.number + 1).checked_add(skip) {
                            block = next.into()
                        } else {
                            break
                        }
                    }
                    HeadersDirection::Falling => {
                        if skip > 0 {
                            // prevent under flows for block.number == 0 and `block.number - skip <
                            // 0`
                            if let Some(next) =
                                header.number.checked_sub(1).and_then(|num| num.checked_sub(skip))
                            {
                                block = next.into()
                            } else {
                                break
                            }
                        } else {
                            block = header.parent_hash.into()
                        }
                    }
                }

                headers.push(header);

                if headers.len() >= MAX_HEADERS_SERVE {
                    break
                }

                total_bytes += APPROX_HEADER_SIZE;

                if total_bytes > SOFT_RESPONSE_LIMIT {
                    break
                }
            } else {
                break
            }
        }

        headers
    }

    fn on_headers_request(
        &mut self,
        _peer_id: PeerId,
        request: GetBlockHeaders,
        response: oneshot::Sender<RequestResult<BlockHeaders>>,
    ) {
        self.metrics.received_headers_requests.increment(1);
        let headers = self.get_headers_response(request);
        let _ = response.send(Ok(BlockHeaders(headers)));
    }

    fn on_bodies_request(
        &mut self,
        _peer_id: PeerId,
        request: GetBlockBodies,
        response: oneshot::Sender<RequestResult<BlockBodies>>,
    ) {
        self.metrics.received_bodies_requests.increment(1);
        let mut bodies = Vec::new();

        let mut total_bytes = APPROX_BODY_SIZE;

        for hash in request.0 {
            if let Some(block) = self.client.block_by_hash(hash).unwrap_or_default() {
                let body = BlockBody {
                    transactions: block.body,
                    ommers: block.ommers,
                    withdrawals: block.withdrawals,
                };

                bodies.push(body);

                total_bytes += APPROX_BODY_SIZE;

                if total_bytes > SOFT_RESPONSE_LIMIT {
                    break
                }

                if bodies.len() >= MAX_BODIES_SERVE {
                    break
                }
            } else {
                break
            }
        }

        let _ = response.send(Ok(BlockBodies(bodies)));
    }

    fn on_receipts_request(
        &mut self,
        _peer_id: PeerId,
        request: GetReceipts,
        response: oneshot::Sender<RequestResult<Receipts>>,
    ) {
        let mut receipts = Vec::new();

        let mut total_bytes = APPROX_RECEIPT_SIZE;

        for hash in request.0 {
            if let Some(receipts_by_block) =
                self.client.receipts_by_block(BlockHashOrNumber::Hash(hash)).unwrap_or_default()
            {
                receipts.push(
                    receipts_by_block
                        .into_iter()
                        .map(|receipt| receipt.with_bloom())
                        .collect::<Vec<_>>(),
                );

                total_bytes += APPROX_RECEIPT_SIZE;

                if total_bytes > SOFT_RESPONSE_LIMIT {
                    break
                }

                if receipts.len() >= MAX_RECEIPTS_SERVE {
                    break
                }
            } else {
                break
            }
        }

        let _ = response.send(Ok(Receipts(receipts)));
    }
}

/// An endless future.
///
/// This should be spawned or used as part of `tokio::select!`.
impl<C> Future for EthRequestHandler<C>
where
    C: BlockReader + HeaderProvider + Unpin,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            match this.incoming_requests.poll_next_unpin(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(incoming)) => match incoming {
                    IncomingEthRequest::GetBlockHeaders { peer_id, request, response } => {
                        this.on_headers_request(peer_id, request, response)
                    }
                    IncomingEthRequest::GetBlockBodies { peer_id, request, response } => {
                        this.on_bodies_request(peer_id, request, response)
                    }
                    IncomingEthRequest::GetNodeData { .. } => {}
                    IncomingEthRequest::GetReceipts { peer_id, request, response } => {
                        this.on_receipts_request(peer_id, request, response)
                    }
                },
            }
        }
    }
}

/// Represents a handled [`GetBlockHeaders`] requests
///
/// This is the key type for spam detection cache. The counter is ignored during `PartialEq` and
/// `Hash`.
#[derive(Debug, PartialEq, Hash)]
#[allow(unused)]
struct RespondedGetBlockHeaders {
    req: (PeerId, GetBlockHeaders),
}

impl Borrow<(PeerId, GetBlockHeaders)> for RespondedGetBlockHeaders {
    fn borrow(&self) -> &(PeerId, GetBlockHeaders) {
        &self.req
    }
}

/// All `eth` request related to blocks delegated by the network.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum IncomingEthRequest {
    /// Request Block headers from the peer.
    ///
    /// The response should be sent through the channel.
    GetBlockHeaders {
        peer_id: PeerId,
        request: GetBlockHeaders,
        response: oneshot::Sender<RequestResult<BlockHeaders>>,
    },
    /// Request Block headers from the peer.
    ///
    /// The response should be sent through the channel.
    GetBlockBodies {
        peer_id: PeerId,
        request: GetBlockBodies,
        response: oneshot::Sender<RequestResult<BlockBodies>>,
    },
    /// Request Node Data from the peer.
    ///
    /// The response should be sent through the channel.
    GetNodeData {
        peer_id: PeerId,
        request: GetNodeData,
        response: oneshot::Sender<RequestResult<NodeData>>,
    },
    /// Request Receipts from the peer.
    ///
    /// The response should be sent through the channel.
    GetReceipts {
        peer_id: PeerId,
        request: GetReceipts,
        response: oneshot::Sender<RequestResult<Receipts>>,
    },
}
