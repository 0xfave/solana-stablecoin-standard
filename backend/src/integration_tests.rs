#[cfg(test)]
mod tests {
    use crate::services::rpc::RpcClient;
    use crate::services::mint_burn::{MintBurnService, MintBurnConfig};
    use crate::services::compliance::ComplianceService;
    use crate::services::events::{EventIndexer, OnChainEvent, EventType};
    use crate::services::solana::{SolanaService, PROGRAM_ID};

    const DEVNET_RPC: &str = "https://api.devnet.solana.com";

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

    #[test]
    fn test_solana_service_initialize_and_mint() {
        let _ = dotenvy::dotenv();
        let private_key = std::env::var("PRIVATE_KEY_BASE64")
            .expect("PRIVATE_KEY_BASE64 not set");
        
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| DEVNET_RPC.to_string());
        
        let service = SolanaService::new(&rpc_url, &private_key)
            .expect("Failed to create Solana service");
        
        println!("Initializing new stablecoin...");
        let result = service.initialize(0, Some(1_000_000_000_000), 6);
        
        let (mint, config) = match result {
            Ok((m, c)) => {
                println!("✓ Initialize successful!");
                println!("  Mint: {}", m);
                println!("  Config: {}", c);
                (m, c)
            }
            Err(e) => {
                println!("⚠ Initialize failed: {}", e);
                panic!("Initialize failed: {}", e);
            }
        };
        
        let recipient = service.payer_pubkey();
        
        println!("Minting 1 token to {}...", recipient);
        let mint_result = service.mint_tokens(&mint, &recipient, 1);
        
        if mint_result.success {
            println!("✓ Mint successful: {}", mint_result.signature);
        } else {
            println!("⚠ Mint failed: {:?}", mint_result.error);
            panic!("Mint failed: {:?}", mint_result.error);
        }
    }

    #[tokio::test]
    async fn test_instruction_discriminators() {
        use sha2::{Sha256, Digest};
        
        fn get_disc(name: &str) -> [u8; 8] {
            let mut hasher = Sha256::new();
            hasher.update(format!("global:{}", name));
            let result = hasher.finalize();
            let mut discriminator = [0u8; 8];
            discriminator.copy_from_slice(&result[..8]);
            discriminator
        }
        
        let mint_disc = get_disc("mint");
        assert_eq!(mint_disc.len(), 8);
        println!("Mint discriminator: {:?}", mint_disc);
        
        let burn_disc = get_disc("burn");
        assert_eq!(burn_disc.len(), 8);
        println!("Burn discriminator: {:?}", burn_disc);
        
        let transfer_disc = get_disc("transfer");
        assert_eq!(transfer_disc.len(), 8);
        println!("Transfer discriminator: {:?}", transfer_disc);
    }
}
