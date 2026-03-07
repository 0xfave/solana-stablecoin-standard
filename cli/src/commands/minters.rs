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

#[derive(Subcommand)]
pub enum MinterAction {
    /// List all current minters
    List,
    /// Add a new minter address
    Add { address: String },
    /// Remove an existing minter address
    Remove { address: String },
}

fn discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    h.to_bytes()[..8].try_into().unwrap()
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    action: MinterAction,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;

    let (config, _) =
        Pubkey::find_program_address(&[b"stablecoin", &mint_pubkey.to_bytes()], &program_id);

    match action {
        MinterAction::List => {
            let response = rpc.get_account(&config.to_string()).await?;

            let encoded = response
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.get("data"))
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Config account not found"))?;

            let decoded = base64::engine::general_purpose::STANDARD.decode(encoded)?;

            // StablecoinConfig Borsh layout (no preset, no transfer_hook, no blacklister):
            //   [0..8]   discriminator
            //   [8..40]  master_authority       (Pubkey)
            //   [40..72] mint                   (Pubkey)
            //   [72]     paused                 (bool)
            //   [73]     supply_cap flag        (0=None, 1=Some)
            //   [74..82] supply_cap value       (u64, only if flag=1)
            //   next     decimals               (u8)
            //   next     bump                   (u8)
            //   next     pending_authority flag (0=None, 1=Some)
            //   +0|32    pending_authority      (Pubkey, only if flag=1)
            //   next     minters: Vec<Pubkey>   (4-byte LE length + N×32)
            //   next     freezer                (Pubkey)
            //   next     pauser                 (Pubkey)

            let mut offset = 73usize;

            // supply_cap: Option<u64>
            if decoded.len() <= offset {
                anyhow::bail!("Account data too short");
            }
            let supply_cap_flag = decoded[offset]; offset += 1;
            if supply_cap_flag == 1 { offset += 8; }

            // decimals + bump
            offset += 2;

            // pending_master_authority: Option<Pubkey>
            if decoded.len() <= offset {
                anyhow::bail!("Account data too short at pending_authority");
            }
            let pending_flag = decoded[offset]; offset += 1;
            if pending_flag == 1 { offset += 32; }

            // minters: Vec<Pubkey>
            if decoded.len() < offset + 4 {
                anyhow::bail!("Account data too short to read minters length");
            }
            let minter_count = u32::from_le_bytes(
                decoded[offset..offset + 4].try_into()?
            ) as usize;
            offset += 4;

            if decoded.len() < offset + minter_count * 32 {
                anyhow::bail!("Account data too short to read {} minters", minter_count);
            }

            let mut minters = Vec::with_capacity(minter_count);
            for _ in 0..minter_count {
                let pk = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);
                minters.push(pk);
                offset += 32;
            }

            // freezer
            if decoded.len() < offset + 32 {
                anyhow::bail!("Account data too short to read freezer");
            }
            let freezer = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);
            offset += 32;

            // pauser
            if decoded.len() < offset + 32 {
                anyhow::bail!("Account data too short to read pauser");
            }
            let pauser = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);

            println!("Stablecoin Roles:");
            println!("  Config:      {}", config);
            println!("  Minters ({}):", minter_count);
            if minters.is_empty() {
                println!("    (none)");
            } else {
                for m in &minters {
                    println!("    - {}", m);
                }
            }
            println!("  Freezer:     {}", freezer);
            println!("  Pauser:      {}", pauser);
            println!("  Note: Blacklister is stored on the compliance module PDA.");
        }

        MinterAction::Add { address } => {
            println!("Adding minter: {}", address);

            let new_minter = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let mut data = discriminator("add_minter").to_vec();
            data.extend_from_slice(&new_minter.to_bytes());

            let ix = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(config, false),
                    AccountMeta::new(authority, true),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));
            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("✅ Minter added: {}", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }

        MinterAction::Remove { address } => {
            println!("Removing minter: {}", address);

            let minter = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            let mut data = discriminator("remove_minter").to_vec();
            data.extend_from_slice(&minter.to_bytes());

            let ix = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(config, false),
                    AccountMeta::new(authority, true),
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));
            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("✅ Minter removed: {}", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }
    }

    Ok(())
}
