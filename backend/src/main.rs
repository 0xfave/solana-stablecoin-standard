use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};

mod routes;
mod services;

#[cfg(test)]
mod integration_tests;

use routes::health::health;
use routes::webhook::webhook;
use services::compliance::{AuditEntry, BlacklistEntry, ComplianceService};
use services::events::{EventIndexer, EventListener, ListenerConfig, OnChainEvent};
use services::mint_burn::{BurnRequest, MintBurnConfig, MintBurnService, MintRequest};
use services::rpc::RpcClient;
use services::webhook::{WebhookConfig, WebhookService};

// ─── Tier ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Tier {
    Sss1,
    Sss2,
    Sss3,
}

impl Tier {
    fn from_env() -> Self {
        match std::env::var("SSS_TIER").as_deref() {
            Ok("SSS-2") | Ok("sss-2") | Ok("2") => Tier::Sss2,
            Ok("SSS-3") | Ok("sss-3") | Ok("3") => Tier::Sss3,
            _ => Tier::Sss1,
        }
    }
}

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub rpc: Arc<RpcClient>,
    pub program_id: Pubkey,
    pub mint: Pubkey,
    pub tier: Tier,
    pub mint_burn: Arc<RwLock<MintBurnService>>,
    pub compliance: Arc<ComplianceService>,
    pub indexer: Arc<RwLock<EventIndexer>>,
}

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CreateMintRequest {
    pub user_wallet: String,
    pub amount: u64,
    pub fiat_tx_id: String,
    pub custodian: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateBurnRequest {
    pub user_wallet: String,
    pub token_account: String,
    pub amount: u64,
    pub fiat_destination: String,
    pub custodian: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BlacklistAddRequest {
    pub address: String,
    pub reason: String,
    pub added_by: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct GetEventsQuery {
    pub event_type: Option<String>,
    pub processed: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, serde::Deserialize)]
pub struct AuditExportQuery {
    pub from: Option<String>, // ISO 8601
    pub to: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ApiResponse<T: serde::Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: serde::Serialize> ApiResponse<T> {
    fn ok(data: T) -> Json<Self> {
        Json(Self { success: true, data: Some(data), error: None })
    }
    fn err(msg: impl Into<String>) -> Json<Self> {
        Json(Self { success: false, data: None, error: Some(msg.into()) })
    }
}

// ─── Core routes (all tiers) ──────────────────────────────────────────────────

async fn get_info(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    ApiResponse::ok(serde_json::json!({
        "program_id": state.program_id.to_string(),
        "mint": state.mint.to_string(),
        "tier": state.tier,
    }))
}

async fn create_mint_request(
    State(state): State<AppState>,
    Json(payload): Json<CreateMintRequest>,
) -> Json<ApiResponse<MintRequest>> {
    // Pre-screen wallet at SSS-2/3
    if state.tier != Tier::Sss1 {
        let check = state.compliance.check_address(&payload.user_wallet).await;
        if !check.allowed {
            return ApiResponse::err(check.reason.unwrap_or_else(|| "Address blocked".to_string()));
        }
    }
    let service = state.mint_burn.read().await;
    match service
        .create_mint_request(
            payload.user_wallet.clone(),
            payload.amount,
            payload.fiat_tx_id.clone(),
            payload.custodian.clone(),
        )
        .await
    {
        Ok(req) => ApiResponse::ok(req),
        Err(e) => ApiResponse::err(e),
    }
}

async fn get_mint_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<MintRequest>> {
    match state.mint_burn.read().await.get_mint_request(&id).await {
        Some(r) => ApiResponse::ok(r),
        None => ApiResponse::err("Mint request not found"),
    }
}

async fn get_mints_by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Json<ApiResponse<Vec<MintRequest>>> {
    ApiResponse::ok(state.mint_burn.read().await.get_mints_by_wallet(&wallet).await)
}

async fn create_burn_request(
    State(state): State<AppState>,
    Json(payload): Json<CreateBurnRequest>,
) -> Json<ApiResponse<BurnRequest>> {
    if state.tier != Tier::Sss1 {
        let check = state.compliance.check_address(&payload.user_wallet).await;
        if !check.allowed {
            return ApiResponse::err(check.reason.unwrap_or_else(|| "Address blocked".to_string()));
        }
    }
    let service = state.mint_burn.read().await;
    match service
        .create_burn_request(
            payload.user_wallet.clone(),
            payload.token_account.clone(),
            payload.amount,
            payload.fiat_destination.clone(),
            payload.custodian.clone(),
        )
        .await
    {
        Ok(req) => ApiResponse::ok(req),
        Err(e) => ApiResponse::err(e),
    }
}

async fn get_burn_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<BurnRequest>> {
    match state.mint_burn.read().await.get_burn_request(&id).await {
        Some(r) => ApiResponse::ok(r),
        None => ApiResponse::err("Burn request not found"),
    }
}

async fn get_burns_by_wallet(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Json<ApiResponse<Vec<BurnRequest>>> {
    ApiResponse::ok(state.mint_burn.read().await.get_burns_by_wallet(&wallet).await)
}

async fn get_events(
    State(state): State<AppState>,
    Query(q): Query<GetEventsQuery>,
) -> Json<ApiResponse<Vec<services::events::IndexedEvent>>> {
    let indexer = state.indexer.read().await;
    let mut events = indexer.get_all_events().clone();
    if let Some(ref t) = q.event_type {
        events.retain(|e| format!("{:?}", e.event_type) == *t);
    }
    if let Some(p) = q.processed {
        events.retain(|e| e.processed == p);
    }
    if let Some(limit) = q.limit {
        events.truncate(limit);
    }
    ApiResponse::ok(events)
}

async fn get_events_by_signature(
    State(state): State<AppState>,
    Path(sig): Path<String>,
) -> Json<ApiResponse<Vec<services::events::IndexedEvent>>> {
    let indexer = state.indexer.read().await;
    ApiResponse::ok(indexer.get_events_by_signature(&sig).into_iter().cloned().collect::<Vec<_>>())
}

// ─── Compliance routes (SSS-2 / SSS-3) ───────────────────────────────────────

async fn check_address(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    let result = state.compliance.check_address(&address).await;
    ApiResponse::ok(serde_json::json!({
        "address": address,
        "allowed": result.allowed,
        "reason": result.reason,
        "rules_triggered": result.rules_triggered,
        "risk_score": result.risk_score,
    }))
}

async fn get_blacklist(State(state): State<AppState>) -> Json<ApiResponse<Vec<BlacklistEntry>>> {
    ApiResponse::ok(state.compliance.get_blacklist().await)
}

async fn add_to_blacklist(
    State(state): State<AppState>,
    Json(payload): Json<BlacklistAddRequest>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state
        .compliance
        // ✅ Matches compliance.rs signature: (String, String, String)
        .add_to_blacklist(payload.address.clone(), payload.reason.clone(), payload.added_by.clone())
        .await
    {
        Ok(_) => ApiResponse::ok(serde_json::json!({ "blacklisted": payload.address })),
        Err(e) => ApiResponse::err(e),
    }
}

async fn remove_from_blacklist(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    // ✅ Matches compliance.rs signature: (address: &str, remover: &str)
    match state.compliance.remove_from_blacklist(&address, "api").await {
        Ok(_) => ApiResponse::ok(serde_json::json!({ "removed": address })),
        Err(e) => ApiResponse::err(e),
    }
}

async fn check_transaction_compliance(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Json<ApiResponse<serde_json::Value>> {
    let from = payload["from"].as_str().unwrap_or("");
    let to = payload["to"].as_str().unwrap_or("");
    let amount = payload["amount"].as_u64().unwrap_or(0);
    let result = state.compliance.check_transaction(from, to, amount).await;
    ApiResponse::ok(serde_json::json!({
        "allowed": result.allowed,
        "reason": result.reason,
        "rules_triggered": result.rules_triggered,
        "risk_score": result.risk_score,
    }))
}

async fn get_compliance_rules(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<services::compliance::ComplianceRule>>> {
    ApiResponse::ok(state.compliance.get_rules().await)
}

async fn export_audit(
    State(state): State<AppState>,
    Query(q): Query<AuditExportQuery>,
) -> Json<ApiResponse<Vec<AuditEntry>>> {
    let from = q.from.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let to = q.to.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    ApiResponse::ok(state.compliance.export_audit_log(from, to).await)
}

async fn get_stats(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let indexer = state.indexer.read().await;
    let all = indexer.get_all_events();
    let blacklist = state.compliance.get_blacklist().await;
    ApiResponse::ok(serde_json::json!({
        "total_events": all.len(),
        "processed": all.iter().filter(|e| e.processed).count(),
        "pending": all.iter().filter(|e| !e.processed).count(),
        "blacklisted_addresses": blacklist.iter().filter(|e| e.status == services::compliance::BlacklistStatus::Active).count(),
    }))
}

// ─── Event processor ─────────────────────────────────────────────────────────

async fn event_processor(
    mut receiver: mpsc::Receiver<OnChainEvent>,
    indexer: Arc<RwLock<EventIndexer>>,
    webhooks: Arc<RwLock<WebhookService>>,
    compliance: Arc<ComplianceService>,
) {
    while let Some(event) = receiver.recv().await {
        info!("Processing event: {:?}", event.event_type);

        let mut idx = indexer.write().await;
        idx.add_event(event.clone());
        drop(idx);

        // Record all on-chain events to the compliance audit trail
        compliance.record_transaction(services::compliance::TransactionMonitor {
            transaction_id: event.signature.clone(),
            from: String::new(),
            to: String::new(),
            amount: 0,
            timestamp: event.timestamp,
            status: services::compliance::TransactionStatus::Completed,
            compliance_result: None,
        }).await;

        let wh = webhooks.read().await;
        if wh.is_enabled() {
            if let Err(e) = wh.send_event(event).await {
                warn!("Webhook delivery failed: {}", e);
            }
        }
    }
}

// ─── Router ───────────────────────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    let tier = state.tier.clone();

    let core = Router::new()
        .route("/health", get(health))
        .route("/webhook", post(webhook))
        .route("/api/info", get(get_info))
        .route("/api/mint", post(create_mint_request))
        .route("/api/mint/:id", get(get_mint_request))
        .route("/api/mint/wallet/:wallet", get(get_mints_by_wallet))
        .route("/api/burn", post(create_burn_request))
        .route("/api/burn/:id", get(get_burn_request))
        .route("/api/burn/wallet/:wallet", get(get_burns_by_wallet))
        .route("/api/events", get(get_events))
        .route("/api/events/:signature", get(get_events_by_signature));

    let compliance = if tier == Tier::Sss2 || tier == Tier::Sss3 {
        Router::new()
            .route("/api/compliance/check/:address", get(check_address))
            .route("/api/compliance/check-tx", post(check_transaction_compliance))
            .route("/api/compliance/blacklist", get(get_blacklist))
            .route("/api/compliance/blacklist", post(add_to_blacklist))
            .route("/api/compliance/blacklist/:address", delete(remove_from_blacklist))
            .route("/api/compliance/rules", get(get_compliance_rules))
            .route("/api/compliance/audit", get(export_audit))
            .route("/api/compliance/stats", get(get_stats))
    } else {
        Router::new()
    };

    core.merge(compliance).with_state(state)
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    let ws_url = std::env::var("WS_URL")
        .unwrap_or_else(|_| "wss://api.devnet.solana.com".to_string());
    let program_id: Pubkey = std::env::var("PROGRAM_ID")
        .unwrap_or_else(|_| "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw".to_string())
        .parse().expect("Invalid PROGRAM_ID");
    let mint: Pubkey = std::env::var("MINT_ADDRESS")
        .expect("MINT_ADDRESS is required")
        .parse().expect("Invalid MINT_ADDRESS");
    let minter: Pubkey = std::env::var("MINTER_ADDRESS")
        .unwrap_or_else(|_| mint.to_string())
        .parse().expect("Invalid MINTER_ADDRESS");
    let decimals: u8 = std::env::var("DECIMALS").unwrap_or_else(|_| "6".to_string())
        .parse().unwrap_or(6);
    let max_supply: u64 = std::env::var("MAX_SUPPLY")
        .unwrap_or_else(|_| "1000000000000000".to_string())
        .parse().unwrap_or(1_000_000_000_000_000);
    let port: u16 = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string())
        .parse().unwrap_or(3000);
    let tier = Tier::from_env();

    info!("Starting SSS Backend — tier: {:?}, program: {}, mint: {}", tier, program_id, mint);

    let rpc = Arc::new(RpcClient::new(rpc_url.clone()));
    let mint_burn = Arc::new(RwLock::new(MintBurnService::new(MintBurnConfig {
        mint, minter, decimals, max_supply, confirmation_timeout_secs: 30,
    })));
    let compliance = Arc::new(ComplianceService::new(
        std::env::var("SANCTIONS_API_URL").ok(),
    ));
    let indexer = Arc::new(RwLock::new(EventIndexer::new()));
    let (event_tx, event_rx) = mpsc::channel::<OnChainEvent>(1000);

    let listener = EventListener::new(
        ListenerConfig {
            program_id: program_id.to_string(),
            ws_url: ws_url.clone(),
            rpc_url: rpc_url.clone(),
            commitment: "confirmed".to_string(),
            filter_depth: 100,
        },
        event_tx,
        rpc.clone(),
    );

    let webhook_service = Arc::new(RwLock::new(WebhookService::new(WebhookConfig {
        url: std::env::var("WEBHOOK_URL").unwrap_or_default(),
        secret: std::env::var("WEBHOOK_SECRET").ok(),
        retry_count: 3,
        retry_delay_ms: 1000,
        enabled: std::env::var("WEBHOOK_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true",
    })));

    let state = AppState {
        rpc,
        program_id,
        mint,
        tier,
        mint_burn,
        compliance: compliance.clone(),
        indexer: indexer.clone(),
    };

    let app = build_router(state);

    tokio::spawn(async move {
        if let Err(e) = listener.start().await {
            error!("Event listener failed: {}", e);
        }
    });
    tokio::spawn(event_processor(event_rx, indexer, webhook_service, compliance));

    let tcp = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    info!("Listening on http://0.0.0.0:{}", port);
    axum::serve(tcp, app).await.unwrap();
}
