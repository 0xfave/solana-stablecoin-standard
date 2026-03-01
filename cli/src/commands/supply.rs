use crate::rpc_client::RpcClient;
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub async fn execute(rpc: &RpcClient, mint: Option<String>) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;

    let _mint = Pubkey::from_str(&mint_str)?;

    let supply = rpc.get_token_supply(&mint_str).await?;

    if let Some(result) = supply.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(amount) = value.get("amount") {
                println!("Total Supply (raw): {}", amount);
            }
            if let Some(ui_amount) = value.get("uiAmountString") {
                println!("Total Supply (UI): {}", ui_amount);
            }
            if let Some(decimals) = value.get("decimals") {
                println!("Decimals: {}", decimals);
            }
        }
    } else {
        println!("Error: Could not fetch supply");
    }

    Ok(())
}
