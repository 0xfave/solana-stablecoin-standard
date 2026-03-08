use base64::Engine;
use reqwest::Client;
use serde_json::Value;
use solana_sdk::{commitment_config::CommitmentConfig, hash::Hash, pubkey::Pubkey, transaction::Transaction};
use std::str::FromStr;

pub struct RpcClient {
    http_client: Client,
    pub url: String,
    pub commitment: CommitmentConfig,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        RpcClient {
            http_client: Client::new(),
            url,
            commitment: CommitmentConfig::confirmed(),
        }
    }

    async fn call(&self, method: &str, params: serde_json::Value) -> Result<Value, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let response = self
            .http_client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let json: Value = response.json().await.map_err(|e| e.to_string())?;
        if let Some(err) = json.get("error") {
            return Err(format!("RPC error: {}", err));
        }
        Ok(json)
    }

    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Vec<u8>, String> {
        let json = self
            .call("getAccountInfo", serde_json::json!([pubkey.to_string(), { "encoding": "base64" }]))
            .await?;
        let data = json
            .get("result").and_then(|r| r.get("value"))
            .and_then(|v| v.get("data")).and_then(|d| d.as_array())
            .and_then(|arr| arr.first()).and_then(|v| v.as_str())
            .ok_or("No account data")?;
        base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())
    }

    /// Returns the full raw JSON response — used by TUI fetch_status.
    pub async fn get_account_raw(&self, pubkey_str: &str) -> Result<Value, String> {
        self.call("getAccountInfo", serde_json::json!([pubkey_str, { "encoding": "base64" }])).await
    }

    pub async fn get_account_json(&self, pubkey_str: &str) -> Result<Value, String> {
        let json = self
            .call("getAccountInfo", serde_json::json!([pubkey_str, { "encoding": "jsonParsed" }]))
            .await?;
        json.get("result").cloned().ok_or_else(|| "No result".to_string())
    }

    pub async fn get_latest_blockhash(&self) -> Result<Hash, String> {
        let json = self
            .call("getLatestBlockhash", serde_json::json!([{ "commitment": "confirmed" }]))
            .await?;
        let hash_str = json
            .get("result").and_then(|r| r.get("value"))
            .and_then(|v| v.get("blockhash")).and_then(|h| h.as_str())
            .ok_or("No blockhash")?;
        Hash::from_str(hash_str).map_err(|e| e.to_string())
    }

    pub async fn send_transaction(&self, tx: &Transaction) -> Result<String, anyhow::Error> {
        // bincode is a transitive dep of solana_sdk — add `bincode = "1"` to Cargo.toml
        let serialized = bincode::serialize(tx)
            .map_err(|e| anyhow::anyhow!("Serialize failed: {}", e))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&serialized);
        let json = self
            .call("sendTransaction", serde_json::json!([encoded, {
                "encoding": "base64",
                "preflightCommitment": "confirmed",
                "skipPreflight": false,
            }]))
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        json.get("result")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No signature in response: {}", json))
    }

    pub async fn get_token_supply(&self, mint: &str) -> Result<Value, String> {
        self.call("getTokenSupply", serde_json::json!([mint, { "commitment": "confirmed" }])).await
    }

    pub async fn get_transaction_json(&self, signature: &str, commitment: &str) -> Result<Value, String> {
        let json = self
            .call("getTransaction", serde_json::json!([signature, { "encoding": "json", "commitment": commitment }]))
            .await?;
        json.get("result").cloned().ok_or_else(|| "No result".to_string())
    }

    pub async fn get_signatures_for_address(&self, address: &Pubkey, limit: usize) -> Result<Vec<Value>, String> {
        let json = self
            .call("getSignaturesForAddress", serde_json::json!([address.to_string(), { "limit": limit, "commitment": "confirmed" }]))
            .await?;
        json.get("result").and_then(|r| r.as_array()).cloned()
            .ok_or_else(|| "No result".to_string())
    }

    pub async fn get_program_accounts(&self, program_id: &str) -> Result<Vec<Value>, String> {
        let json = self
            .call("getProgramAccounts", serde_json::json!([program_id, { "encoding": "base64" }]))
            .await?;
        json.get("result").and_then(|r| r.as_array()).cloned()
            .ok_or_else(|| "No result".to_string())
    }
}
