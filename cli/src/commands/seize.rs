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
    address: &str,
    to: &str,
    amount: u64,
    mint: Option<String>,
) -> Result<()> {
    println!("Seizing {} tokens from {} to {}", amount, address, to);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str(&signer::get_token_2022_program_id())?;
    let from = Pubkey::from_str(address)?;
    let to_pubkey = Pubkey::from_str(to)?;
    let authority = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint = Pubkey::from_str(&mint_str)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint.to_bytes()],
        &program_id,
    );

    let (blacklist, _) = Pubkey::find_program_address(
        &[b"blacklist", &config.to_bytes(), &from.to_bytes()],
        &mint,
    );

    let amount_bytes = amount.to_le_bytes();
    let mut data = vec![10u8];
    data.extend_from_slice(&amount_bytes);

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(from, false),
            AccountMeta::new(to_pubkey, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(blacklist, false),
            AccountMeta::new_readonly(token_2022, false),
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));

    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Seized {} tokens from {} to {}", amount, address, to);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
