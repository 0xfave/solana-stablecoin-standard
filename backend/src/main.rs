use axum::{
    routing::{get, post},
    extract::{Path, State, Query},
    Json, Router,
};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, error, warn};
use tracing_subscriber;
use serde::Deserialize;

mod routes;
mod services;

use routes::health::health;
use routes::webhook::webhook;
use services::rpc::RpcClient;
use services::mint_burn::{MintBurnService, MintBurnConfig, MintRequest, BurnRequest};
use services::compliance::ComplianceService;
use services::events::{EventIndexer, EventListener, ListenerConfig, OnChainEvent};
use services::webhook::{WebhookManager, WebhookConfig, WebhookService};

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub program_id: Pubkey,
    pub mint_burn: Arc<RwLock<MintBurnService>>,
    pub compliance: Arc<ComplianceService>,
    pub indexer: Arc<RwLock<EventIndexer>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMintRequest {
    pub user_wallet: String,
    pub amount: u64,
    pub fiat_tx_id: String,
    pub custodian: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateBurnRequest {
    pub user_wallet: String,
    pub token_account: String,
    pub amount: u64,
    pub fiat_destination: String,
    pub custodian: String,
}

#[derive(serde::Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
    
    fn error(msg: &str) -> Self {
        Self { success: false, data: None, error: Some(msg.to_string()) }
    }
}

async fn create_mint_request(
    State(state): State<AppState>,
    Json(payload): Json<CreateMintRequest>,
) -> Json<ApiResponse<MintRequest>> {
    let service = state.mint_burn.read().await;
    
    match service.create_mint_request(
        payload.user_wallet,
        payload.amount,
        payload.fiat_tx_id,
        payload.custodian,
    ).await {
        Ok(request) => Json(ApiResponse::success(request)),
        Err(e) => Json(ApiResponse::error(&e)),
    }
}

async fn get_mint_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<MintRequest>> {
    let service = state.mint_burn.read().await;
    
    match service.get_mint_request(&id).await {
        Some(request) => Json(ApiResponse::success(request)),
        None => Json(ApiResponse::error("Mint request not found")),
    }
}

async fn get_mints_by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Json<ApiResponse<Vec<MintRequest>>> {
    let service = state.mint_burn.read().await;
    let requests = service.get_mints_by_wallet(&wallet).await;
    Json(ApiResponse::success(requests))
}

async fn create_burn_request(
    State(state): State<AppState>,
    Json(payload): Json<CreateBurnRequest>,
) -> Json<ApiResponse<BurnRequest>> {
    let service = state.mint_burn.read().await;
    
    match service.create_burn_request(
        payload.user_wallet,
        payload.token_account,
        payload.amount,
        payload.fiat_destination,
        payload.custodian,
    ).await {
        Ok(request) => Json(ApiResponse::success(request)),
        Err(e) => Json(ApiResponse::error(&e)),
    }
}

async fn get_burn_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<BurnRequest>> {
    let service = state.mint_burn.read().await;
    
    match service.get_burn_request(&id).await {
        Some(request) => Json(ApiResponse::success(request)),
        None => Json(ApiResponse::error("Burn request not found")),
    }
}

async fn get_burns_by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Json<ApiResponse<Vec<BurnRequest>>> {
    let service = state.mint_burn.read().await;
    let requests = service.get_burns_by_wallet(&wallet).await;
    Json(ApiResponse::success(requests))
}

async fn check_blacklist(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    let result = state.compliance.check_address(&address).await;
    
    Json(ApiResponse::success(serde_json::json!({
        "address": address,
        "allowed": result.allowed,
        "reason": result.reason,
        "rules_triggered": result.rules_triggered,
        "risk_score": result.risk_score,
    })))
}

async fn get_blacklist(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<services::compliance::BlacklistEntry>>> {
    let entries = state.compliance.get_blacklist().await;
    Json(ApiResponse::success(entries))
}

#[derive(Deserialize)]
struct GetEventsQuery {
    event_type: Option<String>,
    processed: Option<bool>,
}

async fn get_events(
    State(state): State<AppState>,
    Query(query): Query<GetEventsQuery>,
) -> Json<ApiResponse<Vec<services::events::IndexedEvent>>> {
    let indexer = state.indexer.read().await;
    let events = indexer.get_all_events().clone();
    
    let filtered: Vec<_> = events.into_iter().filter(|e| {
        let type_match = query.event_type.as_ref().map_or(true, |t| {
            format!("{:?}", e.event_type) == *t
        });
        let processed_match = query.processed.map_or(true, |p| e.processed == p);
        type_match && processed_match
    }).collect();
    
    Json(ApiResponse::success(filtered))
}

async fn get_events_by_signature(
    State(state): State<AppState>,
    Path(signature): Path<String>,
) -> Json<ApiResponse<Vec<services::events::IndexedEvent>>> {
    let indexer = state.indexer.read().await;
    let events = indexer.get_events_by_signature(&signature);
    let collected: Vec<_> = events.into_iter().cloned().collect();
    Json(ApiResponse::success(collected))
}

async fn event_processor(
    mut receiver: mpsc::Receiver<OnChainEvent>,
    indexer: Arc<RwLock<EventIndexer>>,
    webhooks: Arc<RwLock<WebhookService>>,
) {
    while let Some(event) = receiver.recv().await {
        info!("Processing event: {:?}", event.event_type);
        
        let mut idx = indexer.write().await;
        idx.add_event(event.clone());
        drop(idx);
        
        let wh = webhooks.read().await;
        if wh.is_enabled() {
            if let Err(e) = wh.send_event(event).await {
                warn!("Failed to send webhook: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "http://localhost:8899".to_string());
    let ws_url = std::env::var("WS_URL").unwrap_or_else(|_| "ws://localhost:8900".to_string());
    let program_id_str = std::env::var("PROGRAM_ID").unwrap_or_else(|_| "SSSysT1WSh3HPg1GqJL3iKzR5P5vV4F6xT2N9K8mP1".to_string());
    let program_id = program_id_str.parse::<Pubkey>().unwrap_or_default();

    info!("Starting SSS Backend with program: {}", program_id);

    let rpc = Arc::new(RpcClient::new(rpc_url.clone()));

    let mint_burn_config = MintBurnConfig {
        mint: program_id,
        minter: program_id,
        decimals: 6,
        max_supply: 1_000_000_000_000,
        confirmation_timeout_secs: 30,
    };
    let mint_burn_service = MintBurnService::new(mint_burn_config);
    let mint_burn = Arc::new(RwLock::new(mint_burn_service));

    let compliance = Arc::new(ComplianceService::new(None));
    let indexer = Arc::new(RwLock::new(EventIndexer::new()));

    let (event_tx, event_rx) = mpsc::channel::<OnChainEvent>(1000);

    let listener_config = ListenerConfig {
        program_id: program_id.to_string(),
        ws_url: ws_url.clone(),
        rpc_url: rpc_url.clone(),
        commitment: "confirmed".to_string(),
        filter_depth: 100,
    };
    let listener = EventListener::new(
        listener_config,
        event_tx,
        rpc.clone(),
    );

    let webhook_config = WebhookConfig {
        url: std::env::var("WEBHOOK_URL").unwrap_or_default(),
        secret: std::env::var("WEBHOOK_SECRET").ok(),
        retry_count: 3,
        retry_delay_ms: 1000,
        enabled: std::env::var("WEBHOOK_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true",
    };
    
    let webhook_service = Arc::new(RwLock::new(WebhookService::new(webhook_config)));

    let state = AppState {
        rpc: rpc.clone(),
        program_id,
        mint_burn: mint_burn.clone(),
        compliance: compliance.clone(),
        indexer: indexer.clone(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/webhook", post(webhook))
        .route("/api/mint", post(create_mint_request))
        .route("/api/mint/:id", get(get_mint_request))
        .route("/api/mint/wallet/:wallet", get(get_mints_by_wallet))
        .route("/api/burn", post(create_burn_request))
        .route("/api/burn/:id", get(get_burn_request))
        .route("/api/burn/wallet/:wallet", get(get_burns_by_wallet))
        .route("/api/blacklist/check/:address", get(check_blacklist))
        .route("/api/blacklist", get(get_blacklist))
        .route("/api/events", get(get_events))
        .route("/api/events/:signature", get(get_events_by_signature))
        .with_state(state);

    tokio::spawn(async move {
        if let Err(e) = listener.start().await {
            error!("Event listener failed: {}", e);
        }
    });

    tokio::spawn(event_processor(
        event_rx,
        indexer,
        webhook_service,
    ));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    info!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
