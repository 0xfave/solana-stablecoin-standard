use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

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

    let discriminator: [u8; 8] = [0x4e, 0xec, 0x55, 0x68, 0xa9, 0xe7, 0xcd, 0x59];
    let mut data = discriminator.to_vec();
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
