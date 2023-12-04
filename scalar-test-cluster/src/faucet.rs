struct AppState {
    faucet: Arc<dyn FaucetClient + Sync + Send>,
}

async fn start_faucet(cluster: &LocalNewCluster, port: u16) -> Result<()> {
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

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

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
