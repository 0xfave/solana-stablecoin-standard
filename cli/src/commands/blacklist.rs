use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use base64::Engine;
use clap::Subcommand;
use solana_sdk::hash::hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

fn discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    h.to_bytes()[..8].try_into().unwrap()
}

fn borsh_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    out
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    action: BlacklistAction,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let authority = keypair.pubkey();

    let (config, _) =
        Pubkey::find_program_address(&[b"stablecoin", &mint_pubkey.to_bytes()], &program_id);

    // Compliance module PDA — required for all blacklist operations
    let (compliance_module, _) =
        Pubkey::find_program_address(&[b"compliance", &config.to_bytes()], &program_id);

    match action {
        BlacklistAction::Add { address, reason } => {
            let reason_str = reason.unwrap_or_else(|| "No reason provided".to_string());
            println!("Adding {} to blacklist: {}", address, reason_str);

            let system_program = Pubkey::from_str(&signer::get_system_program_id())?;
            let target = Pubkey::from_str(&address)?;

            let (blacklist, _) = Pubkey::find_program_address(
                &[b"blacklist", &config.to_bytes(), &target.to_bytes()],
                &program_id,
            );

            println!("Config:           {}", config);
            println!("Compliance PDA:   {}", compliance_module);
            println!("Blacklist PDA:    {}", blacklist);

            // Check if already blacklisted
            if let Ok(response) = rpc.get_account(&blacklist.to_string()).await {
                if let Some(value) = response.get("result").and_then(|r| r.get("value")) {
                    if !value.is_null() {
                        println!("Address {} is already blacklisted!", address);
                        return Ok(());
                    }
                }
            }

            let mut data = discriminator("blacklist_add").to_vec();
            data.extend_from_slice(&borsh_string(&reason_str));

            // accounts: [blacklist_entry, compliance_module, config, blacklister, target, system_program]
            let ix = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(blacklist, false),                // blacklist_entry (writable)
                    AccountMeta::new_readonly(compliance_module, false), // compliance_module
                    AccountMeta::new_readonly(config, false),           // config
                    AccountMeta::new(authority, true),                  // blacklister (writable, signer)
                    AccountMeta::new_readonly(target, false),           // target wallet
                    AccountMeta::new_readonly(system_program, false),   // system_program
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));
            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("✅ Added {} to blacklist", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }

        BlacklistAction::Remove { address } => {
            println!("Removing {} from blacklist", address);

            let target = Pubkey::from_str(&address)?;

            let (blacklist, _) = Pubkey::find_program_address(
                &[b"blacklist", &config.to_bytes(), &target.to_bytes()],
                &program_id,
            );

            println!("Config:           {}", config);
            println!("Compliance PDA:   {}", compliance_module);
            println!("Blacklist PDA:    {}", blacklist);

            // Check if actually blacklisted
            let is_blacklisted = rpc
                .get_account(&blacklist.to_string())
                .await
                .ok()
                .and_then(|r| r.get("result")?.get("value").cloned())
                .map(|v| !v.is_null())
                .unwrap_or(false);

            if !is_blacklisted {
                println!("Address {} is not blacklisted!", address);
                return Ok(());
            }

            let data = discriminator("blacklist_remove").to_vec();

            // accounts: [blacklist_entry, compliance_module, config, master_authority, target, authority]
            let ix = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(blacklist, false),                // blacklist_entry (writable)
                    AccountMeta::new_readonly(compliance_module, false), // compliance_module
                    AccountMeta::new_readonly(config, false),           // config
                    AccountMeta::new_readonly(authority, true),         // master_authority (signer)
                    AccountMeta::new_readonly(target, false),           // target wallet
                    AccountMeta::new(authority, true),                  // authority (writable, rent recipient)
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));
            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("✅ Removed {} from blacklist", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }

        BlacklistAction::Check { address } => {
            let target = Pubkey::from_str(&address)?;

            let (blacklist, _) = Pubkey::find_program_address(
                &[b"blacklist", &config.to_bytes(), &target.to_bytes()],
                &program_id,
            );

            println!("Blacklist PDA: {}", blacklist);

            let response = rpc.get_account(&blacklist.to_string()).await?;

            let encoded = response
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.get("data"))
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str());

            match encoded {
                None => println!("Account does not exist — not blacklisted"),
                Some(enc) => {
                    let decoded = base64::engine::general_purpose::STANDARD.decode(enc)?;
                    println!("✅ Address is blacklisted. Raw bytes ({}):", decoded.len());
                    println!("  discriminator: {:?}", &decoded[..8]);

                    // BlacklistEntry layout:
                    //   [0..8]   discriminator
                    //   [8..40]  blacklister (Pubkey)
                    //   [40..]   reason (String: 4-byte LE len + bytes)
                    //   next     timestamp (i64)
                    //   next     bump (u8)
                    if decoded.len() > 44 {
                        let reason_len =
                            u32::from_le_bytes(decoded[40..44].try_into()?) as usize;
                        if reason_len > 0 && decoded.len() >= 44 + reason_len {
                            let reason =
                                String::from_utf8_lossy(&decoded[44..44 + reason_len]);
                            println!("  reason: {}", reason);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Subcommand)]
pub enum BlacklistAction {
    Add {
        address: String,
        #[arg(long, short)]
        reason: Option<String>,
    },
    Remove {
        address: String,
    },
    Check {
        address: String,
    },
}
