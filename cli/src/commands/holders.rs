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

    let response = rpc.get_token_accounts(&mint_str).await?;

    if let Some(err) = response.get("error") {
        anyhow::bail!("RPC error: {}", err);
    }

    // result is a direct array (getProgramAccounts), not result.value
    let accounts = response
        .get("result")
        .and_then(|r| r.as_array());

    match accounts {
        None => {
            anyhow::bail!("Unexpected response shape: {}", serde_json::to_string_pretty(&response)?);
        }
        Some(accounts) if accounts.is_empty() => {
            println!("No token holders found.");
        }
        Some(accounts) => {
            let mut holders: Vec<(String, String, u64, String)> = vec![];

            for account in accounts {
                let pubkey = account
                    .get("pubkey")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let info = account
                    .get("account")
                    .and_then(|a| a.get("data"))
                    .and_then(|d| d.get("parsed"))
                    .and_then(|p| p.get("info"));

                let Some(info) = info else { continue };

                let owner = info
                    .get("owner")
                    .and_then(|o| o.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let token_amount = info.get("tokenAmount");

                let amount_raw: u64 = token_amount
                    .and_then(|t| t.get("amount"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);

                let amount_ui = token_amount
                    .and_then(|t| t.get("uiAmountString"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("0")
                    .to_string();

                let state = info
                    .get("state")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                if let Some(min) = min_balance {
                    if amount_raw < min {
                        continue;
                    }
                }

                holders.push((owner, pubkey, amount_raw, amount_ui));
            }

            // Sort by balance descending
            holders.sort_by(|a, b| b.2.cmp(&a.2));

            let total: u64 = holders.iter().map(|h| h.2).sum();

            println!("┌─────────────────────────────────────────────────────────────────────────────────────────────────┐");
            println!("│  Token Holders ({} total)                                                                        ", holders.len());
            println!("├──────┬──────────────────────────────────────────────────┬──────────────────────────────────────────┤");
            println!("│  #   │  Owner (Wallet)                                  │  ATA                                     │");
            println!("├──────┼──────────────────────────────────────────────────┼──────────────────────────────────────────┤");

            for (i, (owner, ata, amount_raw, amount_ui)) in holders.iter().enumerate() {
                println!("│ {:>4} │ {} │ {} │", i + 1, owner, ata);
                println!("│      │  Balance: {:>15} (raw: {:>20})                                   │", amount_ui, amount_raw);
                println!("├──────┼──────────────────────────────────────────────┼──────────────────────────────────────────┤");
            }

            println!("│  Total Supply Held: {:>20} (raw)                                               │", total);
            println!("└─────────────────────────────────────────────────────────────────────────────────────────────────┘");
        }
    }

    Ok(())
}