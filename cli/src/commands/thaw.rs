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
    mint: Option<String>,
) -> Result<()> {
    println!("Thawing account: {}", address);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str(&signer::get_token_2022_program_id())?;
    let account = Pubkey::from_str(address)?;
    let authority = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint = Pubkey::from_str(&mint_str)?;

    let ix = Instruction::new_with_bytes(
        program_id,
        &[5u8],
        vec![
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(account, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(token_2022, false),
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));

    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Thawed account: {}", address);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
