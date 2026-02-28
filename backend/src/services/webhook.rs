use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{info, warn, error};
use reqwest::Client;
use std::collections::HashMap;

use super::events::OnChainEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub url: String,
    pub secret: Option<String>,
    pub retry_count: u32,
    pub retry_delay_ms: u64,
    pub enabled: bool,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            secret: None,
            retry_count: 3,
            retry_delay_ms: 1000,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: OnChainEvent,
    pub webhook_id: String,
    pub attempt: u32,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub success: bool,
    pub message: String,
    pub status_code: u16,
}

pub struct WebhookService {
    client: Client,
    config: WebhookConfig,
    webhook_id: String,
}

impl WebhookService {
    pub fn new(config: WebhookConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            webhook_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub async fn send_event(&self, event: OnChainEvent) -> Result<WebhookResponse, String> {
        if !self.config.enabled {
            info!("Webhook disabled, skipping event: {:?}", event.event_type);
            return Ok(WebhookResponse {
                success: true,
                message: "Webhook disabled".to_string(),
                status_code: 0,
            });
        }

        let payload = WebhookPayload {
            event,
            webhook_id: self.webhook_id.clone(),
            attempt: 1,
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.send_with_retry(payload).await
    }

    async fn send_with_retry(&self, mut payload: WebhookPayload) -> Result<WebhookResponse, String> {
        let mut last_error = String::new();

        for attempt in 1..=self.config.retry_count {
            payload.attempt = attempt;
            
            match self.send_request(&payload).await {
                Ok(response) => {
                    if response.success {
                        info!(
                            "Webhook delivered successfully on attempt {}: {}",
                            attempt, self.config.url
                        );
                        return Ok(response);
                    }
                    last_error = response.message;
                    warn!(
                        "Webhook attempt {} failed: {}",
                        attempt, last_error
                    );
                }
                Err(e) => {
                    last_error = e.to_string();
                    error!("Webhook request error on attempt {}: {}", attempt, e);
                }
            }

            if attempt < self.config.retry_count {
                let delay = Duration::from_millis(self.config.retry_delay_ms * attempt as u64);
                warn!(
                    "Retrying webhook in {:?} (attempt {}/{})",
                    delay, attempt + 1, self.config.retry_count
                );
                sleep(delay).await;
            }
        }

        error!(
            "Webhook failed after {} attempts: {}",
            self.config.retry_count, last_error
        );

        Err(format!(
            "Webhook failed after {} attempts: {}",
            self.config.retry_count, last_error
        ))
    }

    async fn send_request(&self, payload: &WebhookPayload) -> Result<WebhookResponse, String> {
        let mut request = self.client.post(&self.config.url);

        // Add secret header if configured
        if let Some(secret) = &self.config.secret {
            request = request.header("X-Webhook-Secret", secret);
        }

        request = request.header("Content-Type", "application/json");
        request = request.header("X-Webhook-ID", &self.webhook_id);

        let response = request
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let success = status.is_success();

        Ok(WebhookResponse {
            success,
            message: if success {
                "OK".to_string()
            } else {
                format!("HTTP {}", status)
            },
            status_code: status.as_u16(),
        })
    }

    pub fn update_config(&mut self, config: WebhookConfig) {
        info!("Updating webhook config: enabled={}", config.enabled);
        self.config = config;
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[allow(dead_code)]
pub struct WebhookManager {
    webhooks: HashMap<String, Arc<tokio::sync::Mutex<WebhookService>>>,
    event_receiver: mpsc::Receiver<OnChainEvent>,
}

impl WebhookManager {
    pub fn new(event_receiver: mpsc::Receiver<OnChainEvent>) -> Self {
        Self {
            webhooks: HashMap::new(),
            event_receiver,
        }
    }

    pub fn register_webhook(&mut self, name: String, config: WebhookConfig) {
        let service = Arc::new(tokio::sync::Mutex::new(WebhookService::new(config)));
        self.webhooks.insert(name.clone(), service);
        info!("Registered webhook: {}", name);
    }

    pub fn is_enabled(&self) -> bool {
        !self.webhooks.is_empty()
    }

    pub async fn send_event(&self, event: OnChainEvent) -> Result<(), String> {
        for (name, service) in self.webhooks.iter() {
            let service = service.clone();
            let event = event.clone();
            let name = name.clone();
            
            tokio::spawn(async move {
                let service = service.lock().await;
                if let Err(e) = service.send_event(event).await {
                    warn!("Webhook {} failed: {}", name, e);
                }
            });
        }
        Ok(())
    }

    pub async fn start(mut self) {
        info!("Starting webhook manager with {} webhooks", self.webhooks.len());
        
        while let Some(event) = self.event_receiver.recv().await {
            for (name, service) in self.webhooks.iter() {
                let service = service.clone();
                let event = event.clone();
                let name = name.clone();
                
                tokio::spawn(async move {
                    let service = service.lock().await;
                    if let Err(e) = service.send_event(event).await {
                        warn!("Webhook {} failed: {}", name, e);
                    }
                });
            }
        }
    }
}
