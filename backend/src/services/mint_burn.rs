use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};
use solana_sdk::pubkey::Pubkey;

use super::events::{MintEvent, BurnEvent, OnChainEvent, EventType, EventChannel};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MintStatus {
    Pending,
    Processing,
    AwaitingConfirmation,
    Confirmed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintRequest {
    pub id: String,
    pub user_wallet: String,
    pub amount: u64,
    pub fiat_tx_id: String,
    pub custodian: String,
    pub requested_at: DateTime<Utc>,
    pub status: MintStatus,
    pub signature: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BurnStatus {
    Pending,
    Processing,
    AwaitingConfirmation,
    Confirmed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurnRequest {
    pub id: String,
    pub user_wallet: String,
    pub token_account: String,
    pub amount: u64,
    pub fiat_destination: String,
    pub custodian: String,
    pub requested_at: DateTime<Utc>,
    pub status: BurnStatus,
    pub signature: Option<String>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintBurnConfig {
    pub mint: Pubkey,
    pub minter: Pubkey,
    pub decimals: u8,
    pub max_supply: u64,
    pub confirmation_timeout_secs: u64,
}

impl Default for MintBurnConfig {
    fn default() -> Self {
        Self {
            mint: Pubkey::default(),
            minter: Pubkey::default(),
            decimals: 6,
            max_supply: 1_000_000_000_000,
            confirmation_timeout_secs: 30,
        }
    }
}

pub struct MintBurnService {
    config: MintBurnConfig,
    pending_mints: Arc<RwLock<Vec<MintRequest>>>,
    pending_burns: Arc<RwLock<Vec<BurnRequest>>>,
    event_sender: Option<EventChannel>,
}

impl MintBurnService {
    pub fn new(config: MintBurnConfig) -> Self {
        Self {
            config,
            pending_mints: Arc::new(RwLock::new(Vec::new())),
            pending_burns: Arc::new(RwLock::new(Vec::new())),
            event_sender: None,
        }
    }

    pub fn with_event_channel(mut self, sender: EventChannel) -> Self {
        self.event_sender = Some(sender);
        self
    }

    pub fn get_config(&self) -> &MintBurnConfig {
        &self.config
    }

    pub async fn create_mint_request(
        &self,
        user_wallet: String,
        amount: u64,
        fiat_tx_id: String,
        custodian: String,
    ) -> Result<MintRequest, String> {
        if amount == 0 {
            return Err("Amount must be greater than 0".to_string());
        }

        let request = MintRequest {
            id: uuid::Uuid::new_v4().to_string(),
            user_wallet,
            amount,
            fiat_tx_id,
            custodian,
            requested_at: Utc::now(),
            status: MintStatus::Pending,
            signature: None,
            confirmed_at: None,
            error: None,
        };

        info!("Mint request created: {} for {} tokens", request.id, amount);
        
        let mut mints = self.pending_mints.write().await;
        mints.push(request.clone());
        
        Ok(request)
    }

    pub async fn create_burn_request(
        &self,
        user_wallet: String,
        token_account: String,
        amount: u64,
        fiat_destination: String,
        custodian: String,
    ) -> Result<BurnRequest, String> {
        if amount == 0 {
            return Err("Amount must be greater than 0".to_string());
        }

        let request = BurnRequest {
            id: uuid::Uuid::new_v4().to_string(),
            user_wallet,
            token_account,
            amount,
            fiat_destination,
            custodian,
            requested_at: Utc::now(),
            status: BurnStatus::Pending,
            signature: None,
            confirmed_at: None,
            error: None,
        };

        info!("Burn request created: {} for {} tokens", request.id, amount);
        
        let mut burns = self.pending_burns.write().await;
        burns.push(request.clone());
        
        Ok(request)
    }

    pub async fn get_mint_request(&self, request_id: &str) -> Option<MintRequest> {
        let mints = self.pending_mints.read().await;
        mints.iter().find(|m| m.id == request_id).cloned()
    }

    pub async fn get_burn_request(&self, request_id: &str) -> Option<BurnRequest> {
        let burns = self.pending_burns.read().await;
        burns.iter().find(|b| b.id == request_id).cloned()
    }

    pub async fn get_pending_mints(&self) -> Vec<MintRequest> {
        let mints = self.pending_mints.read().await;
        mints.iter().filter(|m| m.status == MintStatus::Pending).cloned().collect()
    }

    pub async fn get_pending_burns(&self) -> Vec<BurnRequest> {
        let burns = self.pending_burns.read().await;
        burns.iter().filter(|b| b.status == BurnStatus::Pending).cloned().collect()
    }

    pub async fn get_all_mints(&self) -> Vec<MintRequest> {
        let mints = self.pending_mints.read().await;
        mints.clone()
    }

    pub async fn get_all_burns(&self) -> Vec<BurnRequest> {
        let burns = self.pending_burns.read().await;
        burns.clone()
    }

    pub async fn update_mint_status(
        &self,
        request_id: &str,
        status: MintStatus,
        signature: Option<String>,
    ) -> Result<MintRequest, String> {
        let mut mints = self.pending_mints.write().await;
        
        let request = mints.iter_mut()
            .find(|m| m.id == request_id)
            .ok_or_else(|| format!("Mint request {} not found", request_id))?;

        request.status = status.clone();
        if let Some(sig) = signature {
            request.signature = Some(sig.clone());
        }
        
        if matches!(status, MintStatus::Confirmed) {
            request.confirmed_at = Some(Utc::now());
            info!("Mint confirmed for request {}", request_id);
            
            if let Some(sender) = &self.event_sender {
                let event = OnChainEvent {
                    event_type: EventType::TokensMinted,
                    signature: request.signature.clone().unwrap_or_default(),
                    slot: 0,
                    timestamp: Utc::now(),
                    data: serde_json::to_value(MintEvent {
                        mint: self.config.mint.to_string(),
                        to: request.user_wallet.clone(),
                        amount: request.amount,
                        minter: self.config.minter.to_string(),
                    }).unwrap(),
                };
                let _ = sender.send(event).await;
            }
        }

        let result = request.clone();
        
        Ok(result)
    }

    pub async fn update_burn_status(
        &self,
        request_id: &str,
        status: BurnStatus,
        signature: Option<String>,
    ) -> Result<BurnRequest, String> {
        let mut burns = self.pending_burns.write().await;
        
        let request = burns.iter_mut()
            .find(|b| b.id == request_id)
            .ok_or_else(|| format!("Burn request {} not found", request_id))?;

        request.status = status.clone();
        if let Some(sig) = signature {
            request.signature = Some(sig.clone());
        }

        let result = request.clone();
        
        if matches!(status, BurnStatus::Confirmed) {
            request.confirmed_at = Some(Utc::now());
            info!("Burn confirmed for request {}", request_id);
            
            if let Some(sender) = &self.event_sender {
                let event = OnChainEvent {
                    event_type: EventType::TokensBurned,
                    signature: request.signature.clone().unwrap_or_default(),
                    slot: 0,
                    timestamp: Utc::now(),
                    data: serde_json::to_value(BurnEvent {
                        mint: self.config.mint.to_string(),
                        from: request.user_wallet.clone(),
                        amount: request.amount,
                        burner: self.config.minter.to_string(),
                    }).unwrap(),
                };
                let _ = sender.send(event).await;
            }
        }

        Ok(result)
    }

    pub async fn fail_mint(&self, request_id: &str, error: String) -> Result<MintRequest, String> {
        let mut mints = self.pending_mints.write().await;
        
        let request = mints.iter_mut()
            .find(|m| m.id == request_id)
            .ok_or_else(|| format!("Mint request {} not found", request_id))?;

        request.status = MintStatus::Failed;
        request.error = Some(error.clone());
        
        warn!("Mint failed for request {}: {}", request_id, error);
        Ok(request.clone())
    }

    pub async fn fail_burn(&self, request_id: &str, error: String) -> Result<BurnRequest, String> {
        let mut burns = self.pending_burns.write().await;
        
        let request = burns.iter_mut()
            .find(|b| b.id == request_id)
            .ok_or_else(|| format!("Burn request {} not found", request_id))?;

        request.status = BurnStatus::Failed;
        request.error = Some(error.clone());
        
        warn!("Burn failed for request {}: {}", request_id, error);
        Ok(request.clone())
    }

    pub async fn cancel_mint(&self, request_id: &str) -> Result<MintRequest, String> {
        let mut mints = self.pending_mints.write().await;
        
        let request = mints.iter_mut()
            .find(|m| m.id == request_id)
            .ok_or_else(|| format!("Mint request {} not found", request_id))?;

        if request.status != MintStatus::Pending {
            return Err("Can only cancel pending requests".to_string());
        }

        request.status = MintStatus::Cancelled;
        info!("Mint cancelled for request {}", request_id);
        Ok(request.clone())
    }

    pub async fn cancel_burn(&self, request_id: &str) -> Result<BurnRequest, String> {
        let mut burns = self.pending_burns.write().await;
        
        let request = burns.iter_mut()
            .find(|b| b.id == request_id)
            .ok_or_else(|| format!("Burn request {} not found", request_id))?;

        if request.status != BurnStatus::Pending {
            return Err("Can only cancel pending requests".to_string());
        }

        request.status = BurnStatus::Cancelled;
        info!("Burn cancelled for request {}", request_id);
        Ok(request.clone())
    }

    pub async fn get_mints_by_wallet(&self, wallet: &str) -> Vec<MintRequest> {
        let mints = self.pending_mints.read().await;
        mints.iter().filter(|m| m.user_wallet == wallet).cloned().collect()
    }

    pub async fn get_burns_by_wallet(&self, wallet: &str) -> Vec<BurnRequest> {
        let burns = self.pending_burns.read().await;
        burns.iter().filter(|b| b.user_wallet == wallet).cloned().collect()
    }

    pub async fn get_mints_by_fiat_tx(&self, fiat_tx_id: &str) -> Option<MintRequest> {
        let mints = self.pending_mints.read().await;
        mints.iter().find(|m| m.fiat_tx_id == fiat_tx_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mint_request_creation() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let result = service.create_mint_request(
            "UserWallet123".to_string(),
            1000,
            "fiat_tx_123".to_string(),
            "custodian_1".to_string(),
        ).await;

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.amount, 1000);
        assert_eq!(request.user_wallet, "UserWallet123");
        assert_eq!(request.status, MintStatus::Pending);
    }

    #[tokio::test]
    async fn test_mint_request_zero_amount() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let result = service.create_mint_request(
            "UserWallet123".to_string(),
            0,
            "fiat_tx_123".to_string(),
            "custodian_1".to_string(),
        ).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_mint_request() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet123".to_string(),
            1000,
            "fiat_tx_123".to_string(),
            "custodian_1".to_string(),
        ).await.unwrap();

        let fetched = service.get_mint_request(&created.id).await;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, created.id);
    }

    #[tokio::test]
    async fn test_get_mints_by_wallet() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        service.create_mint_request(
            "Wallet1".to_string(),
            100,
            "tx1".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        service.create_mint_request(
            "Wallet1".to_string(),
            200,
            "tx2".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        service.create_mint_request(
            "Wallet2".to_string(),
            300,
            "tx3".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        let wallet1_mints = service.get_mints_by_wallet("Wallet1").await;
        assert_eq!(wallet1_mints.len(), 2);

        let wallet2_mints = service.get_mints_by_wallet("Wallet2").await;
        assert_eq!(wallet2_mints.len(), 1);
    }

    #[tokio::test]
    async fn test_update_mint_status() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet".to_string(),
            1000,
            "fiat_tx".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        let updated = service.update_mint_status(
            &created.id,
            MintStatus::Confirmed,
            Some("sig123".to_string()),
        ).await.unwrap();

        assert_eq!(updated.status, MintStatus::Confirmed);
        assert_eq!(updated.signature.unwrap(), "sig123");
        assert!(updated.confirmed_at.is_some());
    }

    #[tokio::test]
    async fn test_fail_mint() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet".to_string(),
            1000,
            "fiat_tx".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        let failed = service.fail_mint(&created.id, "Insufficient funds".to_string()).await.unwrap();

        assert_eq!(failed.status, MintStatus::Failed);
        assert_eq!(failed.error.unwrap(), "Insufficient funds");
    }

    #[tokio::test]
    async fn test_cancel_mint() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet".to_string(),
            1000,
            "fiat_tx".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        let cancelled = service.cancel_mint(&created.id).await.unwrap();
        assert_eq!(cancelled.status, MintStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_mint_already_processed_fails() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet".to_string(),
            1000,
            "fiat_tx".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        service.update_mint_status(&created.id, MintStatus::Processing, None).await.unwrap();

        let result = service.cancel_mint(&created.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_burn_request_creation() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let result = service.create_burn_request(
            "UserWallet123".to_string(),
            "TokenAccount456".to_string(),
            500,
            "bank_account".to_string(),
            "custodian_1".to_string(),
        ).await;

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.amount, 500);
        assert_eq!(request.status, BurnStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_mints_by_fiat_tx() {
        let config = MintBurnConfig::default();
        let service = MintBurnService::new(config);

        let created = service.create_mint_request(
            "UserWallet".to_string(),
            1000,
            "unique_fiat_tx_123".to_string(),
            "custodian".to_string(),
        ).await.unwrap();

        let found = service.get_mints_by_fiat_tx("unique_fiat_tx_123").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, created.id);

        let not_found = service.get_mints_by_fiat_tx("nonexistent").await;
        assert!(not_found.is_none());
    }
}
