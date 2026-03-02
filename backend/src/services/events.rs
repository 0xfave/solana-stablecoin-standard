use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};
use tracing::{info, warn, error, debug};
use serde_json::json;

use super::rpc::RpcClient;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventType {
    ConfigInitialized,
    TokensMinted,
    TokensBurned,
    AccountFrozen,
    AccountThawed,
    AddedToBlacklist,
    RemovedFromBlacklist,
    TokensSeized,
    TransferHookUpdated,
    PausedChanged,
    MinterUpdated,
    FreezerUpdated,
    PauserUpdated,
    BlacklisterUpdated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainEvent {
    pub event_type: EventType,
    pub signature: String,
    pub slot: u64,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintEvent {
    pub mint: String,
    pub to: String,
    pub amount: u64,
    pub minter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnEvent {
    pub mint: String,
    pub from: String,
    pub amount: u64,
    pub burner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TransferEvent {
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub authority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BlacklistEvent {
    pub config: String,
    pub target: String,
    pub reason: Option<String>,
    pub blacklister: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeEvent {
    pub account: String,
    pub mint: String,
    pub freezer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeizeEvent {
    pub mint: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub seizer: String,
}

pub type EventChannel = Sender<OnChainEvent>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenerConfig {
    pub program_id: String,
    pub ws_url: String,
    pub rpc_url: String,
    pub commitment: String,
    pub filter_depth: u32,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            program_id: String::new(),
            ws_url: "wss://api.mainnet-beta.solana.com".to_string(),
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            commitment: "confirmed".to_string(),
            filter_depth: 100,
        }
    }
}

pub struct EventListener {
    config: ListenerConfig,
    event_sender: EventChannel,
    rpc: Arc<RpcClient>,
    seen_signatures: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl EventListener {
    pub fn new(config: ListenerConfig, event_sender: EventChannel, rpc: Arc<RpcClient>) -> Self {
        Self {
            config,
            event_sender,
            rpc,
            seen_signatures: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        loop {
            info!("Starting event listener for program: {}", self.config.program_id);
            
            let ws_url = self.build_ws_url();
            println!("DEBUG: Connecting to websocket: {}", ws_url);
            info!("Connecting to websocket: {}", ws_url);

            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    let (mut write, mut read) = ws_stream.split();

                    let subscribe_msg = self.build_subscribe_message()?;
                    if let Err(e) = write.send(Message::Text(serde_json::to_string(&subscribe_msg)?)).await {
                        error!("Failed to send subscription: {}", e);
                        continue;
                    }
                    
                    info!("Subscribed to program events");

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                if let Err(e) = self.handle_message(&text).await {
                                    warn!("Error handling message: {}", e);
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                if let Err(e) = write.send(Message::Pong(data)).await {
                                    warn!("Failed to send pong: {}", e);
                                }
                            }
                            Ok(Message::Close(_)) => {
                                info!("Websocket connection closed");
                                break;
                            }
                            Err(e) => {
                                error!("Websocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to connect: {}", e);
                }
            }

            warn!("Event listener disconnected, attempting reconnect in 5s...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    fn build_ws_url(&self) -> String {
        self.config.ws_url.clone()
    }

    fn build_subscribe_message(&self) -> Result<serde_json::Value, serde_json::Error> {
        Ok(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [self.config.program_id]
                },
                {
                    "commitment": self.config.commitment
                }
            ]
        }))
    }

    async fn handle_message(&self, text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json: serde_json::Value = serde_json::from_str(text)?;
        
        if let Some(params) = json.get("params") {
            if let Some(result) = params.get("result") {
                if let Some(value) = result.get("value") {
                    if let Some(signature) = value.get("signature").and_then(|s| s.as_str()) {
                        self.process_signature(signature).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_signature(&self, signature: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut seen = self.seen_signatures.write().await;
        
        if seen.contains(signature) {
            return Ok(());
        }
        seen.insert(signature.to_string());
        
        if seen.len() > self.config.filter_depth as usize {
            let to_remove: Vec<_> = seen.iter().take(self.config.filter_depth as usize / 2).cloned().collect();
            for s in to_remove {
                seen.remove(&s);
            }
        }
        drop(seen);

        debug!("Processing signature: {}", signature);

        let signature_str = signature.to_string();
        
        let tx_value: serde_json::Value = self.rpc.get_transaction_json(&signature_str, &self.config.commitment).await
            .map_err(|e| format!("Failed to get transaction: {}", e))?;

        if let Some(log_messages) = tx_value.get("meta").and_then(|m| m.get("logMessages")).and_then(|l| l.as_array()) {
            let slot = tx_value.get("slot").and_then(|s| s.as_u64()).unwrap_or(0);
            
            for log in log_messages {
                if let Some(log_str) = log.as_str() {
                    if let Some(event) = self.parse_log(log_str, &signature_str, slot) {
                        info!("Parsed event: {:?}", event.event_type);
                        if let Err(e) = self.event_sender.send(event).await {
                            warn!("Failed to send event: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_log(&self, log: &str, signature: &str, slot: u64) -> Option<OnChainEvent> {
        if log.contains("ConfigInitialized") {
            return Some(OnChainEvent {
                event_type: EventType::ConfigInitialized,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({}),
            });
        }
        
        if log.contains("TokensMinted") {
            return Some(OnChainEvent {
                event_type: EventType::TokensMinted,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("TokensBurned") {
            return Some(OnChainEvent {
                event_type: EventType::TokensBurned,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("AccountFrozen") {
            return Some(OnChainEvent {
                event_type: EventType::AccountFrozen,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("AccountThawed") {
            return Some(OnChainEvent {
                event_type: EventType::AccountThawed,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("AddedToBlacklist") {
            return Some(OnChainEvent {
                event_type: EventType::AddedToBlacklist,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("RemovedFromBlacklist") {
            return Some(OnChainEvent {
                event_type: EventType::RemovedFromBlacklist,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("TokensSeized") {
            return Some(OnChainEvent {
                event_type: EventType::TokensSeized,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        if log.contains("PausedChanged") {
            return Some(OnChainEvent {
                event_type: EventType::PausedChanged,
                signature: signature.to_string(),
                slot,
                timestamp: Utc::now(),
                data: serde_json::json!({ "note": "Parse instruction data for details" }),
            });
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedEvent {
    pub id: String,
    pub event_type: EventType,
    pub signature: String,
    pub slot: u64,
    pub timestamp: i64,
    pub data: serde_json::Value,
    pub processed: bool,
}

pub struct EventIndexer {
    events: Vec<IndexedEvent>,
}

impl EventIndexer {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn add_event(&mut self, event: OnChainEvent) -> IndexedEvent {
        let indexed = IndexedEvent {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: event.event_type,
            signature: event.signature,
            slot: event.slot,
            timestamp: event.timestamp.timestamp(),
            data: event.data,
            processed: false,
        };
        self.events.push(indexed.clone());
        info!("Indexed event: {:?}", indexed.event_type);
        indexed
    }

    pub fn get_events_by_type(&self, event_type: &EventType) -> Vec<&IndexedEvent> {
        self.events.iter().filter(|e| &e.event_type == event_type).collect()
    }

    pub fn get_events_by_signature(&self, signature: &str) -> Vec<&IndexedEvent> {
        self.events.iter().filter(|e| e.signature == signature).collect()
    }

    pub fn mark_processed(&mut self, event_id: &str) {
        if let Some(event) = self.events.iter_mut().find(|e| e.id == event_id) {
            event.processed = true;
            info!("Marked event {} as processed", event_id);
        }
    }

    pub fn get_unprocessed_events(&self) -> Vec<&IndexedEvent> {
        self.events.iter().filter(|e| !e.processed).collect()
    }

    pub fn get_all_events(&self) -> &Vec<IndexedEvent> {
        &self.events
    }

    pub fn get_events_in_range(&self, from_slot: u64, to_slot: u64) -> Vec<&IndexedEvent> {
        self.events.iter().filter(|e| e.slot >= from_slot && e.slot <= to_slot).collect()
    }
}

impl Default for EventIndexer {
    fn default() -> Self {
        Self::new()
    }
}
