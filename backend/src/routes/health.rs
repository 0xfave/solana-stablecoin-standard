use axum::{extract::State, Json};
use crate::AppState;

pub async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "tier": state.tier,
        "program_id": state.program_id.to_string(),
        "mint": state.mint.to_string(),
        "rpc_url": state.rpc.url,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
