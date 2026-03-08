#[cfg(test)]
mod tests {
    use crate::services::rpc::RpcClient;
    use crate::services::mint_burn::{MintBurnConfig, MintBurnService};
    use crate::services::compliance::ComplianceService;
    use crate::services::events::{EventIndexer, EventType, OnChainEvent};
    use crate::services::solana::PROGRAM_ID;

    const DEVNET_RPC: &str = "https://api.devnet.solana.com";

    #[tokio::test]
    async fn test_rpc_connection() {
        let rpc = RpcClient::new(DEVNET_RPC.to_string());
        let result = rpc.get_account_json("11111111111111111111111111111111").await;
        assert!(result.is_ok(), "Should connect to devnet: {:?}", result);
    }

    #[tokio::test]
    async fn test_get_slot() {
        let client = reqwest::Client::new();
        let response = client
            .post(DEVNET_RPC)
            .json(&serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "getSlot" }))
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
        let result = service
            .create_mint_request("TestWallet123".to_string(), 1000, "fiat_tx_123".to_string(), "custodian".to_string())
            .await;
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

        let add_result = service
            .add_to_blacklist(
                "BlacklistedAddr".to_string(),
                "Test blacklist".to_string(),
                "admin".to_string(),
            )
            .await;
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
            data: serde_json::json!({ "mint": "Mint123", "to": "Wallet456", "amount": 1000 }),
        };

        let indexed = indexer.add_event(event);
        assert!(!indexed.id.is_empty(), "Should have ID");

        let by_sig = indexer.get_events_by_signature("test_sig_123");
        assert_eq!(by_sig.len(), 1, "Should find event by signature");

        let by_type = indexer.get_events_by_type(&EventType::TokensMinted);
        assert_eq!(by_type.len(), 1, "Should find event by type");
    }

    #[tokio::test]
    async fn test_program_query_devnet() {
        let rpc = RpcClient::new(DEVNET_RPC.to_string());
        let result = rpc.get_account_json(PROGRAM_ID).await;
        match &result {
            Ok(json) => {
                if json.get("value").is_some() {
                    println!("✓ Program deployed: {}", PROGRAM_ID);
                } else {
                    println!("⚠ Program not found: {}", PROGRAM_ID);
                }
            }
            Err(e) => println!("⚠ Error querying program: {}", e),
        }
        assert!(result.is_ok(), "Should complete RPC query without error");
    }

    #[test]
    fn test_solana_service_initialize_and_mint() {
        let _ = dotenvy::dotenv();
        let private_key = std::env::var("PRIVATE_KEY_BASE58")
            .expect("PRIVATE_KEY_BASE58 not set");
        let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| DEVNET_RPC.to_string());

        let service = crate::services::solana::SolanaService::new(&rpc_url, &private_key)
            .expect("Failed to create SolanaService");

        println!("Initializing new stablecoin...");
        let (mint, config) = service
            .initialize(Some(1_000_000_000_000), 6)
            .expect("Initialize failed");
        println!("✓ Init: mint={}, config={}", mint, config);

        let recipient = service.payer_pubkey();
        let result = service.mint_tokens(&mint, &recipient, 1);
        assert!(result.success, "Mint failed: {:?}", result.error);
        println!("✓ Mint: {}", result.signature);
    }

    #[test]
    fn test_instruction_discriminators() {
        use sha2::{Digest, Sha256};

        fn disc(name: &str) -> [u8; 8] {
            let mut hasher = Sha256::new();
            hasher.update(format!("global:{}", name));
            hasher.finalize()[..8].try_into().unwrap()
        }

        let cases: &[(&str, [u8; 8])] = &[
            ("mint_tokens",   [0x3b, 0x84, 0x18, 0xf6, 0x7a, 0x27, 0x08, 0xf3]),
            ("burn_tokens",   [0x4c, 0x0f, 0x33, 0xfe, 0xe5, 0xd7, 0x79, 0x42]),
            ("freeze_account",[0xfd, 0x4b, 0x52, 0x85, 0xa7, 0xee, 0x2b, 0x82]),
            ("thaw_account",  [0x73, 0x98, 0x4f, 0xd5, 0xd5, 0xa9, 0xb8, 0x23]),
            ("seize",         [0x81, 0x9f, 0x8f, 0x1f, 0xa1, 0xe0, 0xf1, 0x54]),
        ];

        for (name, expected) in cases {
            let got = disc(name);
            assert_eq!(got, *expected, "Wrong discriminator for '{}'", name);
            println!("✓ {}: {:?}", name, got);
        }
    }
}
