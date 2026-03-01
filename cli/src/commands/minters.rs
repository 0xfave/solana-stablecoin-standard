use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use base64::Engine;
use clap::Subcommand;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

#[derive(Subcommand)]
pub enum MinterAction {
    /// Show the current minter
    List,
    /// Set a new minter address
    Set { address: String },
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

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

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

            // StablecoinConfig Borsh layout:
            //   [0..8]   discriminator
            //   [8..40]  master_authority  (Pubkey)
            //   [40..72] mint              (Pubkey)
            //   [72]     preset            (u8)
            //   [73]     paused            (bool)
            //   [74]     supply_cap flag   (0 = None, 1 = Some)
            //   [75..83] supply_cap value  (u64, only if flag = 1)
            //   next     transfer_hook flag (0 = None, 1 = Some)
            //   +0 or 32 transfer_hook pubkey
            //   next     decimals          (u8)
            //   next     bump              (u8)
            //   next     pending_authority flag
            //   +0 or 32 pending_authority pubkey
            //   next     minter            (Pubkey) ← what we want
            //   next     freezer           (Pubkey)
            //   next     pauser            (Pubkey)
            //   next     blacklister       (Pubkey)

            let mut offset = 74usize;

            // supply_cap: Option<u64>
            let supply_cap_flag = decoded[offset]; offset += 1;
            if supply_cap_flag == 1 { offset += 8; }

            // transfer_hook_program: Option<Pubkey>
            let hook_flag = decoded[offset]; offset += 1;
            if hook_flag == 1 { offset += 32; }

            // decimals + bump
            offset += 2;

            // pending_master_authority: Option<Pubkey>
            let pending_flag = decoded[offset]; offset += 1;
            if pending_flag == 1 { offset += 32; }

            // minter
            if decoded.len() < offset + 32 {
                anyhow::bail!("Account data too short to read minter");
            }
            let minter = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);
            offset += 32;

            // freezer
            let freezer = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);
            offset += 32;

            // pauser
            let pauser = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);
            offset += 32;

            // blacklister
            let blacklister = Pubkey::new_from_array(decoded[offset..offset + 32].try_into()?);

            println!("Stablecoin Roles:");
            println!("  Config:      {}", config);
            println!("  Minter:      {}", minter);
            println!("  Freezer:     {}", freezer);
            println!("  Pauser:      {}", pauser);
            println!("  Blacklister: {}", blacklister);
        }

        MinterAction::Set { address } => {
            println!("Setting minter to {}", address);

            let new_minter = Pubkey::from_str(&address)?;
            let authority = keypair.pubkey();

            // Discriminator from IDL: update_minter = [164, 129, 164, 88, 75, 29, 91, 38]
            // Args: new_minter (Pubkey = 32 bytes, Borsh encoded)
            let discriminator: [u8; 8] = [164, 129, 164, 88, 75, 29, 91, 38];
            let mut data = discriminator.to_vec();
            data.extend_from_slice(&new_minter.to_bytes()); // Pubkey is 32 raw bytes in Borsh

            let ix = Instruction::new_with_bytes(
                program_id,
                &data,
                vec![
                    AccountMeta::new(config, false),            // config (writable)
                    AccountMeta::new_readonly(authority, true), // authority (signer)
                ],
            );

            let tx = Transaction::new_with_payer(&[ix], Some(&authority));
            let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

            println!("Minter updated to: {}", address);
            println!("Signature: {}", signature);
            println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        }
    }

    Ok(())
}