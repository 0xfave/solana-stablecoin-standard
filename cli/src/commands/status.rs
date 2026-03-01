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
                        // StablecoinConfig layout:
                        //   [0..8]   discriminator
                        //   [8..40]  master_authority (Pubkey)
                        //   [40..72] mint (Pubkey)
                        //   [72]     preset (u8)
                        //   [73]     paused (bool)
                        //   [74]     supply_cap option flag (1 = Some, 0 = None)
                        //   [75..83] supply_cap value (u64, only meaningful if flag = 1)
                        if decoded.len() < 74 {
                            println!("  Status: Account data too short ({} bytes)", decoded.len());
                        } else {
                            let authority = Pubkey::new_from_array(
                                decoded[8..40].try_into().unwrap()
                            );
                            let mint_in_config = Pubkey::new_from_array(
                                decoded[40..72].try_into().unwrap()
                            );
                            let preset     = decoded[72];
                            let paused     = decoded[73] != 0;

                            // supply_cap: Option<u64>
                            // Borsh encodes Option<u64> as 1 byte (0/1) + optional 8 bytes
                            let supply_cap = if decoded.len() >= 83 && decoded[74] == 1 {
                                let cap = u64::from_le_bytes(
                                    decoded[75..83].try_into().unwrap()
                                );
                                Some(cap)
                            } else {
                                None
                            };

                            println!("  Authority:    {}", authority);
                            println!("  Mint in cfg:  {}", mint_in_config);
                            println!(
                                "  Preset:       {} (raw: {})",
                                match preset {
                                    0 => "SSS-1",
                                    1 => "SSS-2",
                                    _ => "Unknown",
                                },
                                preset
                            );
                            println!("  Paused:       {}", paused);
                            match supply_cap {
                                Some(cap) => println!("  Supply Cap:   {}", cap),
                                None      => println!("  Supply Cap:   None"),
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