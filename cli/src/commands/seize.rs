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
        &[1u8],
        vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(*token_program, false),
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

    // Derive PDAs
    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

    // Compliance module PDA — required by seize even if just reading
    let (compliance_module, _) = Pubkey::find_program_address(
        &[b"compliance", &config.to_bytes()],
        &program_id,
    );

    // Blacklist PDA for the source wallet
    let (source_blacklist, _) = Pubkey::find_program_address(
        &[b"blacklist", &config.to_bytes(), &source_wallet.to_bytes()],
        &program_id,
    );

    let source_ata = derive_ata(&source_wallet, &mint_pubkey, &token_2022);
    let dest_ata = derive_ata(&dest_wallet, &mint_pubkey, &token_2022);

    println!("Config PDA:       {}", config);
    println!("Compliance PDA:   {}", compliance_module);
    println!("Blacklist PDA:    {}", source_blacklist);
    println!("Source ATA:       {}", source_ata);
    println!("Destination ATA:  {}", dest_ata);

    // Create destination ATA if needed (idempotent)
    let create_dest_ata_ix =
        create_ata_idempotent_ix(&seizer, &dest_wallet, &mint_pubkey, &token_2022);

    // Seize instruction
    // Account order: config, compliance_module, mint, blacklist_pda,
    //                source_ata, dest_ata, authority, token_2022
    let mut data = discriminator("seize").to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let seize_ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new_readonly(config, false),           // config
            AccountMeta::new_readonly(compliance_module, false), // compliance_module
            AccountMeta::new_readonly(mint_pubkey, false),      // mint
            AccountMeta::new_readonly(source_blacklist, false), // blacklist_pda
            AccountMeta::new(source_ata, false),                // source_ata (writable)
            AccountMeta::new(dest_ata, false),                  // dest_ata (writable)
            AccountMeta::new_readonly(seizer, true),            // authority (signer)
            AccountMeta::new_readonly(token_2022, false),       // token_program
        ],
    );

    let tx = Transaction::new_with_payer(&[create_dest_ata_ix, seize_ix], Some(&seizer));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("✅ Seized {} tokens from {} to {}", amount, address, to);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
