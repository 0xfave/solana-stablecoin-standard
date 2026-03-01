use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use clap::Subcommand;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

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
    
    match action {
        BlacklistAction::Add { address, reason } => {
            let reason_str = reason.unwrap_or_else(|| "No reason provided".to_string());
            println!("Adding {} to blacklist: {}", address, reason_str);

            let program_id = Pubkey::from_str(&signer::get_program_id())?;
            let system_program = Pubkey::from_str(&signer::get_system_program_id())?;
            let target = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let (config, _) = Pubkey::find_program_address(
                &[b"stablecoin", &mint_pubkey.to_bytes()],
                &program_id,
            );

            let (blacklist, _) = Pubkey::find_program_address(
                &[b"blacklist", &config.to_bytes(), &target.to_bytes()],
                &mint_pubkey,
            );

            let mut reason_bytes = vec![6u8];
            let reason_padded: Vec<u8> = reason_str.as_bytes().iter().take(199).cloned().collect();
            reason_bytes.extend(reason_padded);
            reason_bytes.resize(200, 0);

            let ix = Instruction::new_with_bytes(
                program_id,
                &reason_bytes,
                vec![
                    AccountMeta::new_readonly(config, false),
                    AccountMeta::new(blacklist, false),
                    AccountMeta::new_readonly(target, false),
                    AccountMeta::new_readonly(authority, true),
                    AccountMeta::new_readonly(system_program, false),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));

            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("Added {} to blacklist", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }
        BlacklistAction::Remove { address } => {
            println!("Removing {} from blacklist", address);

            let program_id = Pubkey::from_str(&signer::get_program_id())?;
            let target = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let (config, _) = Pubkey::find_program_address(
                &[b"stablecoin", &mint_pubkey.to_bytes()],
                &program_id,
            );

            let (blacklist, _) = Pubkey::find_program_address(
                &[b"blacklist", &config.to_bytes(), &target.to_bytes()],
                &mint_pubkey,
            );

            let ix = Instruction::new_with_bytes(
                program_id,
                &[7u8],
                vec![
                    AccountMeta::new_readonly(config, false),
                    AccountMeta::new(blacklist, false),
                    AccountMeta::new_readonly(target, false),
                    AccountMeta::new_readonly(authority, true),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));

            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("Removed {} from blacklist", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
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
}
