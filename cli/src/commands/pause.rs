use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use sha2::{Sha256, Digest};
use solana_sdk::hash::hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

pub fn discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name));
    let result = hasher.finalize();
    result[..8].try_into().unwrap()
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    paused: bool,
    mint: Option<String>,
) -> Result<()> {
    let action = if paused { "Pausing" } else { "Unpausing" };
    println!("{} the stablecoin", action);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let authority = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint = Pubkey::from_str(&mint_str)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint.to_bytes()],
        &program_id,
    );

    let mut data = discriminator("update_paused").to_vec();
    data.push(if paused { 1 } else { 0 });

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new_readonly(authority, true),
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));

    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Done");
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
