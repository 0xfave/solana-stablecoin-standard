use crate::rpc_client::RpcClient;
use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub async fn execute(
    rpc: &RpcClient,
    mint: Option<String>,
    min_balance: Option<u64>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;

    let _mint = Pubkey::from_str(&mint_str)?;

    let accounts = rpc.get_token_accounts(&mint_str).await?;

    println!("Token Holders:");

    if let Some(result) = accounts.get("result") {
        if let Some(value) = result.get("value") {
            if let Some(accounts_array) = value.as_array() {
                if accounts_array.is_empty() {
                    println!("  [no holders found]");
                } else {
                    for account in accounts_array {
                        if let Some(pubkey) = account.get("pubkey") {
                            if let Some(data) = account.get("data").and_then(|d| d.get("parsed")).and_then(|p| p.get("info")) {
                                let _owner = data.get("owner").and_then(|o| o.as_str()).unwrap_or("N/A");
                                let amount = data.get("amount").and_then(|a| a.as_str()).unwrap_or("0");
                                let ui_amount = data.get("uiAmountString").and_then(|u| u.as_str()).unwrap_or(amount);

                                let amount_u64: u64 = amount.parse().unwrap_or(0);
                                if let Some(min) = min_balance {
                                    if amount_u64 >= min {
                                        println!("  {} - {} (raw: {})", pubkey, ui_amount, amount);
                                    }
                                } else {
                                    println!("  {} - {} (raw: {})", pubkey, ui_amount, amount);
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        println!("  [error fetching holders]");
    }

    Ok(())
}
