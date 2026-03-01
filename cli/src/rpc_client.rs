use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use solana_client::rpc_client::RpcClient as SolanaRpcClient;
use solana_sdk::hash::Hash;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;
use std::sync::Arc;

pub struct RpcClient {
    client: Arc<SolanaRpcClient>,
    http_client: Client,
    url: String,
}

impl RpcClient {
    pub fn new(url: String) -> Self {
        let client = SolanaRpcClient::new(url.clone());
        Self {
            client: Arc::new(client),
            http_client: Client::new(),
            url,
        }
    }

    pub async fn get_account(&self, pubkey: &str) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [pubkey, {"encoding": "jsonParsed"}]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn get_token_supply(&self, mint: &str) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTokenSupply",
            "params": [mint]
        });

        let response = self.http_client.post(&self.url)
            .json(&request)
            .send()
            .await?
            .json::<Value>()
            .await?;

        Ok(response)
    }

    pub async fn get_token_accounts(&self, mint: &str) -> Result<Value> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getProgramAccounts",
            "params": [
                "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
                {
                    "encoding": "jsonParsed",
                    "filters": [
                        {
                            "memcmp": {
                                "offset": 0,
                                "bytes": mint,
                                "encoding": "base58"
                            }
                        }
                    ]
                }
            ]
        });
        let response = self.http_client.post(&self.url)
            .json(&body)
            .send()
            .await?
            .json::<Value>()
            .await?;
        Ok(response)
    }

    pub async fn send_transaction<T: Signer + ?Sized>(
        &self,
        mut tx: Transaction,
        signers: &[&T],
    ) -> Result<String> {
        // Get fresh blockhash
        let blockhash = self.client.get_latest_blockhash()?;
        let hash = Hash::from_str(&blockhash.to_string())?;
        
        tx.sign(signers, hash);
        
        // Use the Solana RPC client to send
        let signature = self.client.send_transaction(&tx).map_err(|e| anyhow::anyhow!("{:?}", e))?;
        
        Ok(signature.to_string())
    }

    pub async fn get_latest_blockhash(&self) -> Result<(String, u64)> {
        let blockhash = self.client.get_latest_blockhash()?;
        Ok((blockhash.to_string(), 0))
    }

    pub fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        let lamports = self.client.get_minimum_balance_for_rent_exemption(data_len)?;
        Ok(lamports)
    }

    pub fn get_url(&self) -> &str {
        &self.url
    }
}
