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
    address: &str,
    to: &str,
    amount: u64,
    mint: Option<String>,
) -> Result<()> {
    println!("Seizing {} tokens from {} to {}", amount, address, to);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str(&signer::get_token_2022_program_id())?;
    let source_wallet = Pubkey::from_str(address)?;
    let dest_wallet = Pubkey::from_str(to)?;
    let seizer = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;

    // Derive config PDA
    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

    // Derive blacklist PDA for the source wallet
    // Seeds from IDL: [b"blacklist", config, target] under program_id
    let (source_blacklist, _) = Pubkey::find_program_address(
        &[b"blacklist", &config.to_bytes(), &source_wallet.to_bytes()],
        &program_id,
    );

    // Derive ATAs for source and destination
    let source_ata = derive_ata(&source_wallet, &mint_pubkey, &token_2022);
    let dest_ata = derive_ata(&dest_wallet, &mint_pubkey, &token_2022);

    println!("Source ATA:      {}", source_ata);
    println!("Destination ATA: {}", dest_ata);
    println!("Source blacklist:{}", source_blacklist);

    // Discriminator from IDL: seize = [129, 159, 143, 31, 161, 224, 241, 84]
    let discriminator: [u8; 8] = [129, 159, 143, 31, 161, 224, 241, 84];
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),         // config
            AccountMeta::new_readonly(mint_pubkey, false),    // mint
            AccountMeta::new(source_ata, false),              // source (writable)
            AccountMeta::new(dest_ata, false),                // destination (writable)
            AccountMeta::new_readonly(source_blacklist, false), // source_blacklist
            AccountMeta::new_readonly(seizer, true),          // seizer (signer)
            AccountMeta::new_readonly(token_2022, false),     // token_program
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&seizer));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Seized {} tokens from {} to {}", amount, address, to);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}