use axum::{extract::State, Json};
use serde_json::json;

use crate::AppState;

pub async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "program_id": state.program_id.to_string(),
        "rpc_url": state.rpc.url,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}