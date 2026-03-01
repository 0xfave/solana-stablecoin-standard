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
    println!("  Mint Address: {}", mint);
    println!("  Config Address: {}", config);

    if let Some(result) = config_info.get("result").and_then(|r| r.get("value")) {
        if result.is_null() {
            println!("  Status: Config account not found");
        } else if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
            if let Some(encoded) = data.first().and_then(|v| v.as_str()) {
                use base64::Engine;
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) {
                    if decoded.len() >= 41 {
                        let authority = Pubkey::new_from_array(
                            decoded[8..40].try_into().unwrap()
                        );
                        let preset = decoded[40];
                        println!("  Authority: {}", authority);
                        println!("  Preset: {}", if preset == 0 { "SSS-1" } else { "SSS-2" });
                    }
                }
            }
        }
    } else {
        println!("  Status: Account not found on chain");
    }

    let supply = rpc.get_token_supply(&mint.to_string()).await?;
    if let Some(result) = supply.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(amount) = value.get("amount") {
                println!("  Current Supply: {}", amount);
            }
            if let Some(ui_amount) = value.get("uiAmountString") {
                println!("  UI Amount: {}", ui_amount);
            }
        }
    }

    Ok(())
}
