use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig};
use reqwest::Client;
use serde_json::Value;

pub struct RpcClient {
    http_client: Client,
    pub url: String,
    pub commitment: CommitmentConfig,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        let commitment = CommitmentConfig::confirmed();
        RpcClient { 
            http_client: Client::new(),
            url, 
            commitment 
        }
    }

    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Vec<u8>, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [pubkey.to_string(), {"encoding": "base64"}]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json: Value = response.json().await.map_err(|e| e.to_string())?;
        
        if let Some(result) = json.get("result").and_then(|r| r.get("value")) {
            let data = result.get("data").and_then(|d| d.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .ok_or("No data in response")?;
            
            use base64::Engine;
            let decoded = base64::engine::general_purpose::STANDARD.decode(data)
                .map_err(|e| e.to_string())?;
            Ok(decoded)
        } else {
            Err("Invalid response".to_string())
        }
    }

    pub async fn get_transaction_json(&self, signature: &str, commitment: &str) -> Result<Value, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [signature, {"encoding": "json", "commitment": commitment}]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json: Value = response.json().await.map_err(|e| e.to_string())?;
        
        json.get("result")
            .cloned()
            .ok_or_else(|| "No result in response".to_string())
    }

    pub async fn get_signatures_for_address(&self, address: &Pubkey, limit: usize) -> Result<Vec<Value>, String> {
        let commitment_str = "confirmed";
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignaturesForAddress",
            "params": [address.to_string(), {"limit": limit, "commitment": commitment_str}]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json: Value = response.json().await.map_err(|e| e.to_string())?;
        
        json.get("result")
            .and_then(|r| r.as_array())
            .cloned()
            .ok_or_else(|| "No result in response".to_string())
    }

    pub async fn get_account_json(&self, pubkey_str: &str) -> Result<Value, String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [pubkey_str, {"encoding": "jsonParsed"}]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let json: Value = response.json().await.map_err(|e| e.to_string())?;
        
        json.get("result")
            .cloned()
            .ok_or_else(|| "No result in response".to_string())
    }
}
