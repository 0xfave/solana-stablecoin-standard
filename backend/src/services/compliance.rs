use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum SanctionsStatus {
    Clear,
    Blocked,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistEntry {
    pub address: String,
    pub reason: String,
    pub blacklister: String,
    pub timestamp: DateTime<Utc>,
    pub status: BlacklistStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlacklistStatus {
    Active,
    Removed,
    PendingRemoval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRule {
    pub rule_id: String,
    pub name: String,
    pub enabled: bool,
    pub action: ComplianceAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceAction {
    Allow,
    Block,
    Flag,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMonitor {
    pub transaction_id: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub timestamp: DateTime<Utc>,
    pub status: TransactionStatus,
    pub compliance_result: Option<ComplianceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Compliant,
    Flagged,
    Blocked,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    pub allowed: bool,
    pub reason: Option<String>,
    pub rules_triggered: Vec<String>,
    pub risk_score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub action: AuditAction,
    pub actor: String,
    pub target: String,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    BlacklistAdd,
    BlacklistRemove,
    TransactionBlock,
    TransactionFlag,
    SanctionsScreening,
    RuleUpdate,
}

#[allow(dead_code)]
pub struct ComplianceService {
    blacklist: Arc<RwLock<HashMap<String, BlacklistEntry>>>,
    rules: Arc<RwLock<Vec<ComplianceRule>>>,
    transactions: Arc<RwLock<HashMap<String, TransactionMonitor>>>,
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
    screening_api_url: Option<String>,
}

impl ComplianceService {
    pub fn new(screening_api_url: Option<String>) -> Self {
        let mut rules = Vec::new();

        rules.push(ComplianceRule {
            rule_id: "R001".to_string(),
            name: "Blocked Address Check".to_string(),
            enabled: true,
            action: ComplianceAction::Block,
        });

        rules.push(ComplianceRule {
            rule_id: "R002".to_string(),
            name: "Large Transaction Flag".to_string(),
            enabled: true,
            action: ComplianceAction::Flag,
        });

        rules.push(ComplianceRule {
            rule_id: "R003".to_string(),
            name: "Sanctions Check".to_string(),
            enabled: true,
            action: ComplianceAction::Block,
        });

        Self {
            blacklist: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(rules)),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            screening_api_url,
        }
    }

    pub async fn check_address(&self, address: &str) -> ComplianceResult {
        let blacklist = self.blacklist.read().await;

        if let Some(entry) = blacklist.get(address) {
            if entry.status == BlacklistStatus::Active {
                return ComplianceResult {
                    allowed: false,
                    reason: Some(format!("Address {} is blacklisted: {}", address, entry.reason)),
                    rules_triggered: vec!["R001".to_string()],
                    risk_score: 100,
                };
            }
        }

        ComplianceResult { allowed: true, reason: None, rules_triggered: vec![], risk_score: 0 }
    }

    pub async fn check_transaction(&self, from: &str, to: &str, amount: u64) -> ComplianceResult {
        let mut result = self.check_address(from).await;

        if result.allowed {
            let to_result = self.check_address(to).await;
            if !to_result.allowed {
                return to_result;
            }
        }

        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if !rule.enabled {
                continue;
            }

            if rule.name.contains("Large Transaction") && amount > 10000 {
                result.rules_triggered.push(rule.rule_id.clone());
                result.risk_score += 20;

                if let ComplianceAction::Block = rule.action {
                    result.allowed = false;
                    result.reason = Some(format!("Large transaction flagged: {} tokens", amount));
                }
            }
        }

        result
    }

    pub async fn add_to_blacklist(&self, address: String, reason: String, blacklister: String) -> Result<(), String> {
        let entry = BlacklistEntry {
            address: address.clone(),
            reason: reason.clone(),
            blacklister: blacklister.clone(),
            timestamp: Utc::now(),
            status: BlacklistStatus::Active,
        };

        let mut blacklist = self.blacklist.write().await;
        blacklist.insert(address.clone(), entry);

        self.add_audit_entry(
            AuditAction::BlacklistAdd,
            &blacklister,
            &address,
            serde_json::json!({ "reason": reason }),
            "Success",
        )
        .await;

        info!("Added address to blacklist: {}", address);
        Ok(())
    }

    pub async fn remove_from_blacklist(&self, address: &str, remover: &str) -> Result<(), String> {
        let mut blacklist = self.blacklist.write().await;

        if let Some(entry) = blacklist.get_mut(address) {
            entry.status = BlacklistStatus::Removed;

            self.add_audit_entry(AuditAction::BlacklistRemove, remover, address, serde_json::json!({}), "Success")
                .await;

            info!("Removed address from blacklist: {}", address);
            Ok(())
        } else {
            Err(format!("Address {} not found in blacklist", address))
        }
    }

    pub async fn get_blacklist(&self) -> Vec<BlacklistEntry> {
        let blacklist = self.blacklist.read().await;
        blacklist.values().cloned().collect()
    }

    pub async fn add_rule(&self, rule: ComplianceRule) {
        let mut rules = self.rules.write().await;
        rules.push(rule.clone());
        info!("Added compliance rule: {}", rule.name);
    }

    pub async fn update_rule(&self, rule_id: &str, enabled: bool) -> Result<(), String> {
        let mut rules = self.rules.write().await;

        if let Some(rule) = rules.iter_mut().find(|r| r.rule_id == rule_id) {
            rule.enabled = enabled;
            info!("Updated compliance rule {}: enabled={}", rule_id, enabled);
            Ok(())
        } else {
            Err(format!("Rule {} not found", rule_id))
        }
    }

    pub async fn get_rules(&self) -> Vec<ComplianceRule> {
        let rules = self.rules.read().await;
        rules.clone()
    }

    pub async fn export_audit_log(&self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Vec<AuditEntry> {
        let audit = self.audit_log.read().await;

        audit
            .iter()
            .filter(|entry| {
                let in_range = match (from, to) {
                    (Some(f), Some(t)) => entry.timestamp >= f && entry.timestamp <= t,
                    (Some(f), None) => entry.timestamp >= f,
                    (None, Some(t)) => entry.timestamp <= t,
                    (None, None) => true,
                };
                in_range
            })
            .cloned()
            .collect()
    }

    async fn add_audit_entry(
        &self,
        action: AuditAction,
        actor: &str,
        target: &str,
        details: serde_json::Value,
        result: &str,
    ) {
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action,
            actor: actor.to_string(),
            target: target.to_string(),
            details,
            timestamp: Utc::now(),
            result: result.to_string(),
        };

        let mut audit = self.audit_log.write().await;
        audit.push(entry);
    }

    pub async fn record_transaction(&self, tx: TransactionMonitor) {
        let mut transactions = self.transactions.write().await;
        transactions.insert(tx.transaction_id.clone(), tx);
    }

    pub async fn get_transaction(&self, tx_id: &str) -> Option<TransactionMonitor> {
        let transactions = self.transactions.read().await;
        transactions.get(tx_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_address_clear() {
        let service = ComplianceService::new(None);

        let result = service.check_address("ClearAddress123").await;
        assert!(result.allowed);
        assert_eq!(result.risk_score, 0);
    }

    #[tokio::test]
    async fn test_add_to_blacklist() {
        let service = ComplianceService::new(None);

        let result = service
            .add_to_blacklist("BadActor123".to_string(), "Fraudulent activity".to_string(), "admin".to_string())
            .await;

        assert!(result.is_ok());

        let check = service.check_address("BadActor123").await;
        assert!(!check.allowed);
        assert!(check.reason.is_some());
    }

    #[tokio::test]
    async fn test_remove_from_blacklist() {
        let service = ComplianceService::new(None);

        service
            .add_to_blacklist("AddressToRemove".to_string(), "Reason".to_string(), "admin".to_string())
            .await
            .unwrap();

        let before = service.check_address("AddressToRemove").await;
        assert!(!before.allowed);

        service.remove_from_blacklist("AddressToRemove", "admin").await.unwrap();

        let after = service.check_address("AddressToRemove").await;
        assert!(after.allowed);
    }

    #[tokio::test]
    async fn test_check_transaction_both_clear() {
        let service = ComplianceService::new(None);

        let result = service.check_transaction("Sender123", "Receiver456", 100).await;
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_check_transaction_sender_blacklisted() {
        let service = ComplianceService::new(None);

        service
            .add_to_blacklist("BlacklistedSender".to_string(), "Sanctions".to_string(), "admin".to_string())
            .await
            .unwrap();

        let result = service.check_transaction("BlacklistedSender", "Receiver456", 100).await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_check_transaction_receiver_blacklisted() {
        let service = ComplianceService::new(None);

        service
            .add_to_blacklist("BlacklistedReceiver".to_string(), "Sanctions".to_string(), "admin".to_string())
            .await
            .unwrap();

        let result = service.check_transaction("Sender123", "BlacklistedReceiver", 100).await;
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_large_transaction_flag() {
        let service = ComplianceService::new(None);

        let result = service.check_transaction("Sender123", "Receiver456", 20000).await;
        assert!(result.allowed);
        assert!(result.rules_triggered.iter().any(|r| r == "R002"));
        assert!(result.risk_score > 0);
    }

    #[tokio::test]
    async fn test_get_blacklist() {
        let service = ComplianceService::new(None);

        service.add_to_blacklist("Addr1".to_string(), "Reason1".to_string(), "admin".to_string()).await.unwrap();
        service.add_to_blacklist("Addr2".to_string(), "Reason2".to_string(), "admin".to_string()).await.unwrap();

        let blacklist = service.get_blacklist().await;
        assert_eq!(blacklist.len(), 2);
    }

    #[tokio::test]
    async fn test_add_and_update_rule() {
        let service = ComplianceService::new(None);

        let new_rule = ComplianceRule {
            rule_id: "TEST001".to_string(),
            name: "Test Rule".to_string(),
            enabled: true,
            action: ComplianceAction::Block,
        };

        service.add_rule(new_rule).await;

        let rules = service.get_rules().await;
        assert!(rules.iter().any(|r| r.rule_id == "TEST001"));

        service.update_rule("TEST001", false).await.unwrap();

        let rules_after = service.get_rules().await;
        let test_rule = rules_after.iter().find(|r| r.rule_id == "TEST001").unwrap();
        assert!(!test_rule.enabled);
    }

    #[tokio::test]
    async fn test_export_audit_log() {
        let service = ComplianceService::new(None);

        service.add_to_blacklist("Addr1".to_string(), "Reason".to_string(), "admin".to_string()).await.unwrap();
        service.add_to_blacklist("Addr2".to_string(), "Reason".to_string(), "admin".to_string()).await.unwrap();

        let audit = service.export_audit_log(None, None).await;
        assert!(audit.len() >= 2);
    }
}
