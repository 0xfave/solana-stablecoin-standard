#[cfg(test)]
mod tests {
    use crate::services::rpc::RpcClient;
    use crate::services::mint_burn::{MintBurnService, MintBurnConfig};
    use crate::services::compliance::ComplianceService;
    use crate::services::events::{EventIndexer, OnChainEvent, EventType};

    const DEVNET_RPC: &str = "https://api.devnet.solana.com";
    const PROGRAM_ID: &str = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";

    #[tokio::test]
    async fn test_rpc_connection() {
        let rpc = RpcClient::new(DEVNET_RPC.to_string());
        
        let result = rpc.get_account_json("11111111111111111111111111111111").await;
        assert!(result.is_ok(), "Should connect to devnet: {:?}", result);
    }

    #[tokio::test]
    async fn test_get_slot() {
        let _rpc = RpcClient::new(DEVNET_RPC.to_string());
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
        });
        
        let client = reqwest::Client::new();
        let response = client.post(DEVNET_RPC)
            .json(&request)
            .send()
            .await
            .expect("Should send request");
        
        let json: serde_json::Value = response.json().await.expect("Should parse JSON");
        assert!(json.get("result").is_some(), "Should get slot");
    }

    #[tokio::test]
    async fn test_mint_burn_service() {
        let config = MintBurnConfig {
            mint: PROGRAM_ID.parse().unwrap(),
            minter: PROGRAM_ID.parse().unwrap(),
            decimals: 6,
            max_supply: 1_000_000_000_000,
            confirmation_timeout_secs: 30,
        };
        
        let service = MintBurnService::new(config);
        
        let result = service.create_mint_request(
            "TestWallet123".to_string(),
            1000,
            "fiat_tx_123".to_string(),
            "custodian".to_string(),
        ).await;
        
        assert!(result.is_ok(), "Should create mint request");
        let request = result.unwrap();
        assert_eq!(request.amount, 1000);
        assert_eq!(request.user_wallet, "TestWallet123");
    }

    #[tokio::test]
    async fn test_compliance_service() {
        let service = ComplianceService::new(None);
        
        let result = service.check_address("TestAddress123").await;
        assert!(result.allowed, "Should allow non-blacklisted address");
        
        let add_result = service.add_to_blacklist(
            "BlacklistedAddr".to_string(),
            "Test blacklist".to_string(),
            "admin".to_string(),
        ).await;
        
        assert!(add_result.is_ok(), "Should add to blacklist");
        
        let check_result = service.check_address("BlacklistedAddr").await;
        assert!(!check_result.allowed, "Should block blacklisted address");
    }

    #[tokio::test]
    async fn test_event_indexer() {
        let mut indexer = EventIndexer::new();
        
        let event = OnChainEvent {
            event_type: EventType::TokensMinted,
            signature: "test_sig_123".to_string(),
            slot: 100,
            timestamp: chrono::Utc::now(),
            data: serde_json::json!({
                "mint": "Mint123",
                "to": "Wallet456",
                "amount": 1000,
            }),
        };
        
        let indexed = indexer.add_event(event);
        assert!(!indexed.id.is_empty(), "Should have ID");
        
        let events = indexer.get_events_by_signature("test_sig_123");
        assert_eq!(events.len(), 1, "Should find event");
        
        let mint_events = indexer.get_events_by_type(&EventType::TokensMinted);
        assert_eq!(mint_events.len(), 1, "Should find mint event");
    }

    #[tokio::test]
    async fn test_program_query_devnet() {
        let _rpc = RpcClient::new(DEVNET_RPC.to_string());
        
        let result = _rpc.get_account_json(PROGRAM_ID).await;
        
        match result {
            Ok(ref json) => {
                if json.get("result").and_then(|r| r.get("value")).is_some() {
                    println!("✓ Program is deployed on devnet: {}", PROGRAM_ID);
                } else {
                    println!("⚠ Program not found on devnet: {}", PROGRAM_ID);
                }
            }
            Err(ref e) => {
                println!("⚠ Error querying program: {}", e);
            }
        }
        
        assert!(result.is_ok(), "Should query program");
    }
}
