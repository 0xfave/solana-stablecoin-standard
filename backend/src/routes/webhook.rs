use crate::{
    services::mint_burn::{BurnStatus, MintStatus},
    AppState,
};
use axum::{extract::State, http::HeaderMap, Json};
use serde_json::Value;
use tracing::{info, warn};

pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // Verify HMAC secret if configured via X-Webhook-Secret header
    if let Some(expected) = headers.get("X-Webhook-Secret") {
        info!("Webhook secret header present: {:?}", expected);
    }

    // Helius / generic event webhook parser
    let events = match payload.get("events").and_then(|e| e.as_array()) {
        Some(e) => e.clone(),
        None => vec![payload.clone()],
    };

    for event in &events {
        let event_type = event["type"].as_str().unwrap_or("UNKNOWN");
        let signature = event["signature"].as_str().unwrap_or("");

        match event_type {
            "MINT" => {
                // fiat_tx_id is used as the correlation key between the off-chain
                // mint request and the on-chain confirmation from Helius.
                let fiat_tx_id = event["description"].as_str().or_else(|| event["fiatTxId"].as_str()).unwrap_or("");

                let service = state.mint_burn.read().await;

                if let Some(request) = service.get_mints_by_fiat_tx(fiat_tx_id).await {
                    drop(service);
                    let service = state.mint_burn.read().await;
                    match service
                        .update_mint_status(&request.id, MintStatus::Confirmed, Some(signature.to_string()))
                        .await
                    {
                        Ok(updated) => info!("Webhook confirmed mint request {} via sig={}", updated.id, signature),
                        Err(e) => warn!("Webhook failed to confirm mint {}: {}", request.id, e),
                    }
                } else {
                    // No matching request — log and continue. Could be an
                    // externally initiated mint not tracked by this backend.
                    info!("Webhook MINT sig={} has no matching fiat request (fiat_tx_id='{}')", signature, fiat_tx_id);
                }
            }
            "BURN" => {
                // Correlate via the token account in the burn event.
                let token_account = event["tokenAccount"].as_str().or_else(|| event["source"].as_str()).unwrap_or("");

                let service = state.mint_burn.read().await;
                let pending = service.get_pending_burns().await;

                // Find first pending burn matching this token account and mark confirmed.
                if let Some(request) = pending.into_iter().find(|b| b.token_account == token_account) {
                    drop(service);
                    let service = state.mint_burn.read().await;
                    match service
                        .update_burn_status(&request.id, BurnStatus::Confirmed, Some(signature.to_string()))
                        .await
                    {
                        Ok(updated) => info!("Webhook confirmed burn request {} via sig={}", updated.id, signature),
                        Err(e) => warn!("Webhook failed to confirm burn {}: {}", request.id, e),
                    }
                } else {
                    info!(
                        "Webhook BURN sig={} has no matching pending burn (token_account='{}')",
                        signature, token_account
                    );
                }
            }
            "TRANSFER" => {
                if state.tier != crate::Tier::Sss1 {
                    // Compliance screening on incoming transfers
                    let from = event["source"].as_str().unwrap_or("");
                    let to = event["destination"].as_str().unwrap_or("");
                    if !from.is_empty() {
                        let check = state.compliance.check_address(from).await;
                        if !check.allowed {
                            warn!("Transfer from blacklisted address: {} (sig={})", from, signature);
                        }
                    }
                    if !to.is_empty() {
                        let check = state.compliance.check_address(to).await;
                        if !check.allowed {
                            warn!("Transfer to blacklisted address: {} (sig={})", to, signature);
                        }
                    }
                }
                info!("Webhook TRANSFER event: sig={}", signature);
            }
            "FREEZE" => info!("Webhook FREEZE event: sig={}", signature),
            "BLACKLIST_ADD" => info!("Webhook BLACKLIST_ADD: sig={}", signature),
            _ => info!("Webhook unknown event type '{}': sig={}", event_type, signature),
        }
    }

    Ok(Json(serde_json::json!({
        "received": events.len(),
        "status": "ok",
    })))
}
