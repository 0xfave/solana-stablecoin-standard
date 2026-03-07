use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub async fn execute(rpc: &RpcClient, mint: Option<String>) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint.to_bytes()],
        &program_id,
    );

    let config_info = rpc.get_account(&config.to_string()).await?;

    println!("Stablecoin Status:");
    println!("  Mint Address:   {}", mint);
    println!("  Config Address: {}", config);

    if let Some(result) = config_info.get("result").and_then(|r| r.get("value")) {
        if result.is_null() {
            println!("  Status: Config account not found on chain");
        } else if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
            if let Some(encoded) = data.first().and_then(|v| v.as_str()) {
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(encoded) {
                    Err(_) => println!("  Status: Failed to decode account data"),
                    Ok(decoded) => {
                        // StablecoinConfig layout (no preset field):
                        //   [0..8]   discriminator
                        //   [8..40]  master_authority (Pubkey)
                        //   [40..72] mint (Pubkey)
                        //   [72]     paused (bool)
                        //   [73]     supply_cap option flag (1 = Some, 0 = None)
                        //   [74..82] supply_cap value (u64, only if flag = 1)
                        //   [82]     decimals (u8)
                        //   [83]     bump (u8)
                        if decoded.len() < 74 {
                            println!("  Status: Account data too short ({} bytes)", decoded.len());
                        } else {
                            let authority = Pubkey::new_from_array(
                                decoded[8..40].try_into().unwrap()
                            );
                            let mint_in_config = Pubkey::new_from_array(
                                decoded[40..72].try_into().unwrap()
                            );
                            let paused = decoded[72] != 0;

                            let supply_cap = if decoded.len() >= 82 && decoded[73] == 1 {
                                let cap = u64::from_le_bytes(
                                    decoded[74..82].try_into().unwrap()
                                );
                                Some(cap)
                            } else {
                                None
                            };

                            let decimals = if decoded.len() > 82 { decoded[82] } else { 6 };

                            // Derive compliance and privacy PDAs to show tier
                            let (compliance_pda, _) = Pubkey::find_program_address(
                                &[b"compliance", &config.to_bytes()],
                                &program_id,
                            );
                            let (privacy_pda, _) = Pubkey::find_program_address(
                                &[b"privacy", &config.to_bytes()],
                                &program_id,
                            );

                            let compliance_exists = rpc
                                .get_account(&compliance_pda.to_string())
                                .await
                                .ok()
                                .and_then(|r| r.get("result")?.get("value").cloned())
                                .map(|v| !v.is_null())
                                .unwrap_or(false);

                            let privacy_exists = rpc
                                .get_account(&privacy_pda.to_string())
                                .await
                                .ok()
                                .and_then(|r| r.get("result")?.get("value").cloned())
                                .map(|v| !v.is_null())
                                .unwrap_or(false);

                            let tier = match (compliance_exists, privacy_exists) {
                                (_, true) => "SSS-3 (Privacy)",
                                (true, false) => "SSS-2 (Compliance)",
                                _ => "SSS-1 (Basic)",
                            };

                            println!("  Authority:    {}", authority);
                            println!("  Mint in cfg:  {}", mint_in_config);
                            println!("  Tier:         {}", tier);
                            println!("  Paused:       {}", paused);
                            println!("  Decimals:     {}", decimals);
                            match supply_cap {
                                Some(cap) => println!("  Supply Cap:   {}", cap),
                                None => println!("  Supply Cap:   None"),
                            }

                            if compliance_exists {
                                println!("  Compliance:   {} (attached)", compliance_pda);
                            }
                            if privacy_exists {
                                println!("  Privacy:      {} (attached)", privacy_pda);
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("  Status: Account not found on chain");
    }

    // Token supply
    let supply = rpc.get_token_supply(&mint.to_string()).await?;
    if let Some(result) = supply.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(amount) = value.get("amount") {
                println!("  Current Supply (raw): {}", amount);
            }
            if let Some(ui_amount) = value.get("uiAmountString") {
                println!("  Current Supply (ui):  {}", ui_amount);
            }
        }
    }

    Ok(())
}
