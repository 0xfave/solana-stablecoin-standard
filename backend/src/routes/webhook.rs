use axum::{extract::State, Json};
use serde_json::Value;

use crate::AppState;

pub async fn webhook(
    State(_state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<String, (axum::http::StatusCode, String)> {
    tracing::info!("Received webhook: {:?}", payload);

    // Parse Helius event (example structure)
    if let Some(events) = payload["events"].as_array() {
        for event in events {
            let event_type = event["type"].as_str().unwrap_or("");
            match event_type {
                "MINT" => tracing::info!("Mint event: {:?}", event),
                "TRANSFER" => tracing::info!("Transfer event: {:?}", event),
                "FREEZE" => tracing::info!("Freeze event: {:?}", event),
                _ => {}
            }
            // Add business logic: update DB, trigger compliance, notify issuer
        }
    }

    Ok("Webhook received".to_string())
}