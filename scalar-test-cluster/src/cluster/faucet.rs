// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{FullnodeCluster, FullnodeClusterTrait};
use anyhow::Result;
use async_trait::async_trait;
use axum::{
    extract::Path,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use fastcrypto::encoding::{Encoding, Hex};
use http::{Method, StatusCode};
use std::env;
use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};
use sui_config::{Config, SUI_KEYSTORE_FILENAME};
use sui_faucet::{
    BatchFaucetResponse, BatchStatusFaucetResponse, Faucet, FaucetConfig, FaucetError,
    FaucetRequest, FaucetResponse, FixedAmountRequest, SimpleFaucet,
};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::{
    sui_client_config::{SuiClientConfig, SuiEnv},
    wallet_context::WalletContext,
};
use sui_types::crypto::{KeypairTraits, SuiKeyPair};
use sui_types::{base_types::SuiAddress, crypto::AccountKeyPair};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, info_span, Instrument};
use uuid::Uuid;

pub struct FaucetClientFactory;

impl FaucetClientFactory {
    pub async fn new_from_cluster(
        cluster: &(dyn FullnodeClusterTrait + Sync + Send),
    ) -> Arc<dyn FaucetClient + Sync + Send> {
        match cluster.remote_faucet_url() {
            Some(url) => Arc::new(RemoteFaucetClient::new(url.into())),
            // If faucet_url is none, it's a local cluster
            None => {
                let key = cluster
                    .local_faucet_key()
                    .expect("Expect local faucet key for local cluster")
                    .copy();
                let wallet_context = new_wallet_context_from_cluster(cluster, key)
                    .instrument(info_span!("init_wallet_context_for_faucet"))
                    .await;

                let prom_registry = prometheus::Registry::new();
                let config = FaucetConfig::default();
                let simple_faucet = SimpleFaucet::new(
                    wallet_context,
                    &prom_registry,
                    &cluster.config_directory().join("faucet.wal"),
                    config,
                )
                .await
                .unwrap();

                Arc::new(LocalFaucetClient::new(simple_faucet))
            }
        }
    }
}

/// Faucet Client abstraction
#[async_trait]
pub trait FaucetClient {
    async fn request_sui_coins(&self, request_address: SuiAddress) -> FaucetResponse;
    async fn batch_request_sui_coins(&self, request_address: SuiAddress) -> BatchFaucetResponse;
    async fn get_batch_send_status(&self, task_id: Uuid) -> BatchStatusFaucetResponse;
}

/// Client for a remote faucet that is accessible by POST requests
pub struct RemoteFaucetClient {
    remote_url: String,
}

impl RemoteFaucetClient {
    fn new(url: String) -> Self {
        info!("Use remote faucet: {}", url);
        Self { remote_url: url }
    }
}

#[async_trait]
impl FaucetClient for RemoteFaucetClient {
    /// Request test SUI coins from faucet.
    /// It also verifies the effects are observed by fullnode.
    async fn request_sui_coins(&self, request_address: SuiAddress) -> FaucetResponse {
        let gas_url = format!("{}/gas", self.remote_url);
        debug!("Getting coin from remote faucet {}", gas_url);
        let data = HashMap::from([("recipient", Hex::encode(request_address))]);
        let map = HashMap::from([("FixedAmountRequest", data)]);

        let auth_header = match env::var("FAUCET_AUTH_HEADER") {
            Ok(val) => val,
            _ => "".to_string(),
        };

        let response = reqwest::Client::new()
            .post(&gas_url)
            .header("Authorization", auth_header)
            .json(&map)
            .send()
            .await
            .unwrap_or_else(|e| panic!("Failed to talk to remote faucet {:?}: {:?}", gas_url, e));
        let full_bytes = response.bytes().await.unwrap();
        let faucet_response: FaucetResponse = serde_json::from_slice(&full_bytes)
            .map_err(|e| anyhow::anyhow!("json deser failed with bytes {:?}: {e}", full_bytes))
            .unwrap();

        if let Some(error) = faucet_response.error {
            panic!("Failed to get gas tokens with error: {}", error)
        };

        faucet_response
    }
    async fn batch_request_sui_coins(&self, request_address: SuiAddress) -> BatchFaucetResponse {
        let gas_url = format!("{}/v1/gas", self.remote_url);
        debug!("Getting coin from remote faucet {}", gas_url);
        let data = HashMap::from([("recipient", Hex::encode(request_address))]);
        let map = HashMap::from([("FixedAmountRequest", data)]);

        let auth_header = match env::var("FAUCET_AUTH_HEADER") {
            Ok(val) => val,
            _ => "".to_string(),
        };

        let response = reqwest::Client::new()
            .post(&gas_url)
            .header("Authorization", auth_header)
            .json(&map)
            .send()
            .await
            .unwrap_or_else(|e| panic!("Failed to talk to remote faucet {:?}: {:?}", gas_url, e));
        let full_bytes = response.bytes().await.unwrap();
        let faucet_response: BatchFaucetResponse = serde_json::from_slice(&full_bytes)
            .map_err(|e| anyhow::anyhow!("json deser failed with bytes {:?}: {e}", full_bytes))
            .unwrap();

        if let Some(error) = faucet_response.error {
            panic!("Failed to get gas tokens with error: {}", error)
        };

        faucet_response
    }
    async fn get_batch_send_status(&self, task_id: Uuid) -> BatchStatusFaucetResponse {
        let status_url = format!("{}/v1/status/{}", self.remote_url, task_id);
        debug!(
            "Checking status for task {} from remote faucet {}",
            task_id.to_string(),
            status_url
        );

        let auth_header = match env::var("FAUCET_AUTH_HEADER") {
            Ok(val) => val,
            _ => "".to_string(),
        };

        let response = reqwest::Client::new()
            .get(&status_url)
            .header("Authorization", auth_header)
            .send()
            .await
            .unwrap_or_else(|e| {
                panic!("Failed to talk to remote faucet {:?}: {:?}", status_url, e)
            });
        let full_bytes = response.bytes().await.unwrap();
        let faucet_response: BatchStatusFaucetResponse = serde_json::from_slice(&full_bytes)
            .map_err(|e| anyhow::anyhow!("json deser failed with bytes {:?}: {e}", full_bytes))
            .unwrap();

        faucet_response
    }
}

/// A local faucet that holds some coins since genesis
pub struct LocalFaucetClient {
    simple_faucet: Arc<SimpleFaucet>,
}

impl LocalFaucetClient {
    fn new(simple_faucet: Arc<SimpleFaucet>) -> Self {
        info!("Use local faucet");
        Self { simple_faucet }
    }
}
#[async_trait]
impl FaucetClient for LocalFaucetClient {
    async fn request_sui_coins(&self, request_address: SuiAddress) -> FaucetResponse {
        let receipt = self
            .simple_faucet
            .send(Uuid::new_v4(), request_address, &[200_000_000_000; 5])
            .await
            .unwrap_or_else(|err| panic!("Failed to get gas tokens with error: {}", err));

        receipt.into()
    }
    async fn batch_request_sui_coins(&self, request_address: SuiAddress) -> BatchFaucetResponse {
        let receipt = self
            .simple_faucet
            .batch_send(Uuid::new_v4(), request_address, &[200_000_000_000; 5])
            .await
            .unwrap_or_else(|err| panic!("Failed to get gas tokens with error: {}", err));

        receipt.into()
    }
    async fn get_batch_send_status(&self, task_id: Uuid) -> BatchStatusFaucetResponse {
        let status = self
            .simple_faucet
            .get_batch_send_status(task_id)
            .await
            .unwrap_or_else(|err| panic!("Failed to get gas tokens with error: {}", err));

        status.into()
    }
}

struct AppState {
    faucet: Arc<dyn FaucetClient + Sync + Send>,
}

pub async fn start_faucet(cluster: &FullnodeCluster, port: u16) -> Result<()> {
    let faucet = FaucetClientFactory::new_from_cluster(cluster).await;

    let app_state = Arc::new(AppState { faucet });

    let cors = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    let app = Router::new()
        .route("/", get(health))
        .route("/gas", post(faucet_request))
        .route("/v1/gas", post(faucet_batch_request))
        .route("/v1/status/:task_id", get(request_status))
        .layer(
            ServiceBuilder::new()
                .layer(cors)
                .layer(Extension(app_state))
                .into_inner(),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("Faucet URL: http://{}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

/// basic handler that responds with a static string
async fn health() -> &'static str {
    "OK"
}

async fn faucet_request(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    let result = match payload {
        FaucetRequest::FixedAmountRequest(FixedAmountRequest { recipient }) => {
            state.faucet.request_sui_coins(recipient).await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FaucetResponse::from(FaucetError::Internal(
                    "Input Error.".to_string(),
                ))),
            )
        }
    };

    if !result.transferred_gas_objects.is_empty() {
        (StatusCode::CREATED, Json(result))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(result))
    }
}

async fn faucet_batch_request(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    let result = match payload {
        FaucetRequest::FixedAmountRequest(FixedAmountRequest { recipient }) => {
            state.faucet.batch_request_sui_coins(recipient).await
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(BatchFaucetResponse::from(FaucetError::Internal(
                    "Input Error.".to_string(),
                ))),
            )
        }
    };
    if result.task.is_some() {
        (StatusCode::CREATED, Json(result))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(result))
    }
}

async fn request_status(
    Extension(state): Extension<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match Uuid::parse_str(&id) {
        Ok(task_id) => {
            let status = state.faucet.get_batch_send_status(task_id).await;
            (StatusCode::CREATED, Json(status))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BatchStatusFaucetResponse::from(FaucetError::Internal(
                e.to_string(),
            ))),
        ),
    }
}

pub async fn new_wallet_context_from_cluster(
    cluster: &(dyn FullnodeClusterTrait + Sync + Send),
    key_pair: AccountKeyPair,
) -> WalletContext {
    let config_dir = cluster.config_directory();
    let wallet_config_path = config_dir.join("client.yaml");
    let fullnode_url = cluster.fullnode_url();
    info!("Use RPC: {}", &fullnode_url);
    let keystore_path = config_dir.join(SUI_KEYSTORE_FILENAME);
    let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
    let address: SuiAddress = key_pair.public().into();
    keystore
        .add_key(None, SuiKeyPair::Ed25519(key_pair))
        .unwrap();
    SuiClientConfig {
        keystore,
        envs: vec![SuiEnv {
            alias: "localnet".to_string(),
            rpc: fullnode_url.into(),
            ws: None,
        }],
        active_address: Some(address),
        active_env: Some("localnet".to_string()),
    }
    .persisted(&wallet_config_path)
    .save()
    .unwrap();

    info!(
        "Initialize wallet from config path: {:?}",
        wallet_config_path
    );

    WalletContext::new(&wallet_config_path, None, None)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "Failed to init wallet context from path {:?}, error: {e}",
                wallet_config_path
            )
        })
}
