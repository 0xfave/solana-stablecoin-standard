use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

// Correct Associated Token Program address
const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

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

fn create_ata_idempotent_instruction(
    payer: &Pubkey,
    wallet: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    ata: &Pubkey,
) -> Instruction {
    let associated_token_program = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM).unwrap();
    let system_program = Pubkey::from_str(SYSTEM_PROGRAM).unwrap();

    Instruction::new_with_bytes(
        associated_token_program,
        &[1u8], // 1 = CreateIdempotent
        vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*ata, false),
            AccountMeta::new_readonly(*wallet, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program, false),
            AccountMeta::new_readonly(*token_program, false),
        ],
    )
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    recipient: &str,
    amount: u64,
    mint: Option<String>,
) -> Result<()> {
    println!("Minting {} tokens to {}", amount, recipient);

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let token_2022 = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")?;
    let recipient_pubkey = Pubkey::from_str(recipient)?;
    let authority = keypair.pubkey();

    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );

    let recipient_ata = derive_ata(&recipient_pubkey, &mint_pubkey, &token_2022);
    println!("Recipient ATA: {}", recipient_ata);

    let create_ata_ix = create_ata_idempotent_instruction(
        &authority,
        &recipient_pubkey,
        &mint_pubkey,
        &token_2022,
        &recipient_ata,
    );

    let discriminator: [u8; 8] = [0x33, 0x39, 0xe1, 0x2f, 0xb6, 0x92, 0x89, 0xa6];
    let mut data = discriminator.to_vec();
    data.extend_from_slice(&amount.to_le_bytes());

    let mint_ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config, false),
            AccountMeta::new(mint_pubkey, false),
            AccountMeta::new(recipient_ata, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_2022, false),
        ],
    );

    let tx = Transaction::new_with_payer(&[create_ata_ix, mint_ix], Some(&authority));

    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("Minted {} tokens to {}", amount, recipient);
    println!("Recipient ATA: {}", recipient_ata);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}