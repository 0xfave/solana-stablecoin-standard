use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

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

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    amount: u64,
    mint: Option<String>,
) -> Result<()> {
    println!("Burning {} tokens", amount);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str(&signer::get_token_2022_program_id())?;
    let burner = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;

    // Derive config PDA
    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

    // Derive the burner's ATA — burn draws tokens from this account
    let from_ata = derive_ata(&burner, &mint_pubkey, &token_2022);
    println!("Burning from ATA: {}", from_ata);

    // Discriminator from IDL: burn = [116, 110, 29, 56, 107, 219, 42, 93]
    let discriminator: [u8; 8] = [116, 110, 29, 56, 107, 219, 42, 93];
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),      // config
            AccountMeta::new(mint_pubkey, false),           // mint (writable)
            AccountMeta::new(from_ata, false),             // from (writable)
            AccountMeta::new_readonly(burner, true),       // burner (signer)
            AccountMeta::new_readonly(token_2022, false),  // token_program
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&burner));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Burned {} tokens", amount);
    println!("From ATA: {}", from_ata);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}