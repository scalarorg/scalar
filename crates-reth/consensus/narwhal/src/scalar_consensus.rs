use hyper::body::Bytes;
use jsonrpsee::client_transport::ws::{Url, WsTransportClientBuilder};
use jsonrpsee::core::client::{Client, ClientBuilder, ClientT};
use jsonrpsee::http_client::HttpClientBuilder;
use jsonrpsee::rpc_params;
use jsonrpsee::server::{RpcModule, Server};
use reth_rpc::JwtSecret;
use reth_transaction_pool::{
    PoolConfig, PriceBumpConfig, SubPoolLimit, TransactionListenerKind, TransactionPool,
    DEFAULT_PRICE_BUMP, REPLACE_BLOB_PRICE_BUMP, TXPOOL_MAX_ACCOUNT_SLOTS_PER_SENDER,
    TXPOOL_SUBPOOL_MAX_SIZE_MB_DEFAULT, TXPOOL_SUBPOOL_MAX_TXS_DEFAULT,
};
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing_subscriber::util::SubscriberInitExt;
#[derive(Debug, Clone)]
pub struct ScalarConsensusHandles {}
/// Scalar N&B consensus
///
/// This consensus adapter for listen incommit transaction and send to Consensus Grpc Server.
#[derive(Debug)]
pub struct ScalarConsensus {}

impl ScalarConsensus {
    pub async fn new<Pool>(
        socket_addr: SocketAddr,
        pool: &Pool,
        jwt_secret: JwtSecret,
    ) -> eyre::Result<ScalarConsensusHandles>
    where
        Pool: TransactionPool,
    {
        let transaction_rx =
            pool.new_transactions_listener_for(TransactionListenerKind::PropagateOnly);
        let pending_transaction_rx =
            pool.pending_transactions_listener_for(TransactionListenerKind::PropagateOnly);

        let url = format!("http://{}", socket_addr);

        let middleware = tower::ServiceBuilder::new()
            .layer(
                TraceLayer::new_for_http()
                    .on_request(
                        |request: &hyper::Request<hyper::Body>, _span: &tracing::Span| tracing::info!(request = ?request, "on_request"),
                    )
                    .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                        tracing::info!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                    })
                    .make_span_with(DefaultMakeSpan::new().include_headers(true))
                    .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
            );

        let client = HttpClientBuilder::default()
            .set_middleware(middleware)
            .build(url)?;

        let handles = ScalarConsensusHandles {};
        Ok(handles)
    }
}
