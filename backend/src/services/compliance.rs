use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

        ComplianceResult {
            allowed: true,
            reason: None,
            rules_triggered: vec![],
            risk_score: 0,
        }
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
            "Success"
        ).await;

        info!("Added address to blacklist: {}", address);
        Ok(())
    }

    pub async fn remove_from_blacklist(&self, address: &str, remover: &str) -> Result<(), String> {
        let mut blacklist = self.blacklist.write().await;
        
        if let Some(entry) = blacklist.get_mut(address) {
            entry.status = BlacklistStatus::Removed;
            
            self.add_audit_entry(
                AuditAction::BlacklistRemove,
                remover,
                address,
                serde_json::json!({}),
                "Success"
            ).await;
            
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
        
        audit.iter()
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
