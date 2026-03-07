use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::hash::hash;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

fn discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    h.to_bytes()[..8].try_into().unwrap()
}

pub async fn attach(
    rpc: &RpcClient,
    keypair: &Keypair,
    allowlist_authority: &str,
    confidential: bool,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let system_program = Pubkey::from_str(&signer::get_system_program_id())?;
    let authority = keypair.pubkey();
    let allowlist_auth_pubkey = Pubkey::from_str(allowlist_authority)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );
    let (privacy_module, _) = Pubkey::find_program_address(
        &[b"privacy", &config.to_bytes()],
        &program_id,
    );

    println!("Attaching privacy module...");
    println!("  Config:              {}", config);
    println!("  Privacy PDA:         {}", privacy_module);
    println!("  Allowlist Authority: {}", allowlist_auth_pubkey);
    println!("  Confidential:        {}", confidential);

    let mut data = discriminator("attach_privacy_module").to_vec();
    data.extend_from_slice(&allowlist_auth_pubkey.to_bytes());
    data.push(if confidential { 1 } else { 0 });

    // accounts: [module_pda, config, master_authority, authority, system_program]
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(privacy_module, false),          // module_pda (writable)
            AccountMeta::new_readonly(config, false),         // config
            AccountMeta::new_readonly(authority, true),       // master_authority (signer)
            AccountMeta::new(authority, true),                // authority (writable, payer)
            AccountMeta::new_readonly(system_program, false), // system_program
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("✅ Privacy module attached (SSS-3)");
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}

pub async fn detach(
    rpc: &RpcClient,
    keypair: &Keypair,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let authority = keypair.pubkey();

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );
    let (privacy_module, _) = Pubkey::find_program_address(
        &[b"privacy", &config.to_bytes()],
        &program_id,
    );

    println!("Detaching privacy module...");
    println!("  Config:      {}", config);
    println!("  Privacy PDA: {}", privacy_module);

    let data = discriminator("detach_privacy_module").to_vec();

    // accounts: [module_pda, config, master_authority, authority]
    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(privacy_module, false),     // module_pda (writable, closed)
            AccountMeta::new_readonly(config, false),    // config
            AccountMeta::new_readonly(authority, true),  // master_authority (signer)
            AccountMeta::new(authority, true),           // authority (writable, rent recipient)
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("✅ Privacy module detached");
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}

pub async fn allowlist_add(
    rpc: &RpcClient,
    keypair: &Keypair,
    address: &str,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let system_program = Pubkey::from_str(&signer::get_system_program_id())?;
    let authority = keypair.pubkey();
    let target = Pubkey::from_str(address)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );
    let (privacy_module, _) = Pubkey::find_program_address(
        &[b"privacy", &config.to_bytes()],
        &program_id,
    );
    let (allowlist_entry, _) = Pubkey::find_program_address(
        &[b"allowlist", &privacy_module.to_bytes(), &target.to_bytes()],
        &program_id,
    );

    println!("Adding {} to allowlist...", address);
    println!("  Privacy PDA:    {}", privacy_module);
    println!("  Allowlist PDA:  {}", allowlist_entry);

    let data = discriminator("allowlist_add").to_vec();

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(allowlist_entry, false),         // allowlist_entry (writable)
            AccountMeta::new_readonly(privacy_module, false), // privacy_module
            AccountMeta::new_readonly(config, false),         // config
            AccountMeta::new(authority, true),                // authority (writable, signer)
            AccountMeta::new_readonly(target, false),         // target wallet
            AccountMeta::new_readonly(system_program, false), // system_program
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("✅ Added {} to allowlist", address);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}

pub async fn allowlist_remove(
    rpc: &RpcClient,
    keypair: &Keypair,
    address: &str,
    mint: Option<String>,
) -> Result<()> {
    let mint_str = mint.ok_or_else(|| {
        anyhow::anyhow!("Mint address required. Use --mint flag or set in config.toml")
    })?;
    let mint_pubkey = Pubkey::from_str(&mint_str)?;
    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let authority = keypair.pubkey();
    let target = Pubkey::from_str(address)?;

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint_pubkey.to_bytes()],
        &program_id,
    );
    let (privacy_module, _) = Pubkey::find_program_address(
        &[b"privacy", &config.to_bytes()],
        &program_id,
    );
    let (allowlist_entry, _) = Pubkey::find_program_address(
        &[b"allowlist", &privacy_module.to_bytes(), &target.to_bytes()],
        &program_id,
    );

    println!("Removing {} from allowlist...", address);
    println!("  Privacy PDA:   {}", privacy_module);
    println!("  Allowlist PDA: {}", allowlist_entry);

    let data = discriminator("allowlist_remove").to_vec();

    let ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(allowlist_entry, false),         // allowlist_entry (writable, closed)
            AccountMeta::new_readonly(privacy_module, false), // privacy_module
            AccountMeta::new_readonly(config, false),         // config
            AccountMeta::new_readonly(authority, true),       // master_authority (signer)
            AccountMeta::new_readonly(target, false),         // target wallet
            AccountMeta::new(authority, true),                // authority (writable, rent recipient)
        ],
    );

    let tx = Transaction::new_with_payer(&[ix], Some(&authority));
    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer]).await?;

    println!("✅ Removed {} from allowlist", address);
    println!("Signature: {}", signature);
    println!("Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
