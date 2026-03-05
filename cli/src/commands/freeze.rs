use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;
use solana_sdk::hash::hash;

const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn derive_ata(wallet: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let associated_token_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
    Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            token_program.as_ref(),
            mint.as_ref(),
        ],
        &associated_token_program,
    )
    .0
}

fn discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    h.to_bytes()[..8].try_into().unwrap()
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    address: &str,
    mint: Option<String>,
) -> Result<()> {
    println!("Freezing account: {}", address);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str(&signer::get_token_2022_program_id())?;
    let wallet_pubkey = Pubkey::from_str(address)?;
    let authority = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;

    // Derive config PDA (same seeds as initialize)
    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

    // Derive the wallet's ATA — this is the token account to freeze, not the wallet itself
    let ata = derive_ata(&wallet_pubkey, &mint_pubkey, &token_2022);
    println!("Freezing ATA: {}", ata);

    // Discriminator from IDL: freeze_account = [253, 75, 82, 133, 167, 238, 43, 130]
    let discriminator = discriminator("freeze_account");

    let ix = Instruction::new_with_bytes(
        program_id,
        &discriminator,
        vec![
            AccountMeta::new_readonly(config, false),   // config
            AccountMeta::new_readonly(mint_pubkey, false), // mint
            AccountMeta::new(ata, false),               // account (writable)
            AccountMeta::new_readonly(authority, true), // freezer (signer)
            AccountMeta::new_readonly(token_2022, false), // token_program
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Frozen account: {}", address);
    println!("ATA: {}", ata);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}