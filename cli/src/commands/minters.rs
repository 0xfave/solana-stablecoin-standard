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
    action: MinterAction,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint = Pubkey::from_str(&mint_str)?;
    
    match action {
        MinterAction::List => {
            println!("Minters:");
            println!("  [Query not implemented - use getAccountInfo on config]");
        }
        MinterAction::Add { address } => {
            println!("Adding {} as minter", address);

            let program_id = Pubkey::from_str(&signer::get_program_id())?;
            let minter_pubkey = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let (config, _) = Pubkey::find_program_address(
                &[b"stablecoin", &mint.to_bytes()],
                &program_id,
            );

            let ix = Instruction::new_with_bytes(
                program_id,
                &[11u8],
                vec![
                    AccountMeta::new(config, false),
                    AccountMeta::new_readonly(minter_pubkey, false),
                    AccountMeta::new_readonly(authority, true),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));

            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("Added {} as minter", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }
        MinterAction::Remove { address } => {
            println!("Removing {} from minters", address);

            let program_id = Pubkey::from_str(&signer::get_program_id())?;
            let minter_pubkey = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let (config, _) = Pubkey::find_program_address(
                &[b"stablecoin", &mint.to_bytes()],
                &program_id,
            );

            let ix = Instruction::new_with_bytes(
                program_id,
                &[12u8],
                vec![
                    AccountMeta::new(config, false),
                    AccountMeta::new_readonly(minter_pubkey, false),
                    AccountMeta::new_readonly(authority, true),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));

            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("Removed {} from minters", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }
    }

    Ok(())
}

#[derive(Subcommand)]
pub enum MinterAction {
    List,
    Add {
        address: String,
    },
    Remove {
        address: String,
    },
}
