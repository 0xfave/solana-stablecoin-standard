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
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

fn derive_ata(wallet: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let associated_token_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &associated_token_program,
    )
    .0
}

/// Builds an idempotent create-ATA instruction.
/// Uses discriminator [1] which is the "create idempotent" variant —
/// it succeeds even if the ATA already exists.
fn create_ata_idempotent_ix(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    let associated_token_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
    let system_program = Pubkey::from_str(SYSTEM_PROGRAM).unwrap();
    let ata = derive_ata(owner, mint, token_program);

    Instruction::new_with_bytes(
        associated_token_program,
        &[1u8], // idempotent create
        vec![
            AccountMeta::new(*payer, true),                          // funding account
            AccountMeta::new(ata, false),                            // ATA to create
            AccountMeta::new_readonly(*owner, false),                // ATA owner
            AccountMeta::new_readonly(*mint, false),                 // mint
            AccountMeta::new_readonly(system_program, false),        // system program
            AccountMeta::new_readonly(*token_program, false),        // token program
        ],
    )
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
    let (config, _) =
        Pubkey::find_program_address(&[b"stablecoin", &mint_pubkey.to_bytes()], &program_id);

    // Derive blacklist PDA for source wallet
    let (source_blacklist, _) = Pubkey::find_program_address(
        &[b"blacklist", &config.to_bytes(), &source_wallet.to_bytes()],
        &program_id,
    );

    // Derive ATAs
    let source_ata = derive_ata(&source_wallet, &mint_pubkey, &token_2022);
    let dest_ata = derive_ata(&dest_wallet, &mint_pubkey, &token_2022);

    println!("Source ATA:      {}", source_ata);
    println!("Destination ATA: {}", dest_ata);
    println!("Source blacklist:{}", source_blacklist);

    // Instruction 1: create destination ATA (idempotent — safe if it already exists)
    let create_dest_ata_ix =
        create_ata_idempotent_ix(&seizer, &dest_wallet, &mint_pubkey, &token_2022);

    // Instruction 2: seize
    // Discriminator: sha256("global:seize")[0..8]
    let discriminator: [u8; 8] = [129, 159, 143, 31, 161, 224, 241, 84];
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let seize_ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),              // config
            AccountMeta::new_readonly(mint_pubkey, false),         // mint
            AccountMeta::new(source_ata, false),                   // source (writable)
            AccountMeta::new(dest_ata, false),                     // destination (writable)
            AccountMeta::new_readonly(source_blacklist, false),    // source_blacklist
            AccountMeta::new(seizer, true),                        // seizer (signer, writable for fees)
            AccountMeta::new_readonly(token_2022, false),          // token_program
        ],
    );

    let tx = Transaction::new_with_payer(&[create_dest_ata_ix, seize_ix], Some(&seizer));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Seized {} tokens from {} to {}", amount, address, to);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}