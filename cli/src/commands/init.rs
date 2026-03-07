use crate::config::StablecoinConfig;
use crate::rpc_client::RpcClient;
use crate::signer;
use anyhow::Result;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use solana_sdk::transaction::Transaction;
use std::str::FromStr;

const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

fn compute_discriminator(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let result = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&result[..8]);
    disc
}

pub async fn execute(
    rpc: &RpcClient,
    keypair: &Keypair,
    name_arg: Option<String>,
    symbol_arg: Option<String>,
    decimals_arg: Option<u8>,
    supply_cap: Option<u64>,
    blacklister: Option<String>,
    allowlist_authority: Option<String>,
    config: Option<String>,
) -> Result<()> {
    // Load config from file if provided, otherwise use defaults
    let mut cfg = if let Some(config_path) = config {
        StablecoinConfig::from_file(&config_path)?
    } else {
        StablecoinConfig::default()
    };

    // CLI args override config file values
    if let Some(cap) = supply_cap {
        cfg.supply_cap = Some(cap);
    }
    if let Some(d) = decimals_arg {
        cfg.decimals = d;
    }

    let name = if let Some(n) = name_arg {
        n
    } else {
        println!("\n📛 Enter token name (e.g., 'My USD Coin'): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let n = input.trim().to_string();
        if n.is_empty() { "My Stablecoin".to_string() } else { n }
    };

    let symbol = if let Some(s) = symbol_arg {
        s
    } else {
        println!("\n💰 Enter token symbol (e.g., 'USDC'): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let s = input.trim().to_string();
        if s.is_empty() { "STB".to_string() } else { s }
    };

    let tier = match (&blacklister, &allowlist_authority) {
        (_, Some(_)) => "SSS-3",
        (Some(_), None) => "SSS-2",
        _ => "SSS-1",
    };

    println!("\nInitializing stablecoin:");
    println!("  Name:     {}", name);
    println!("  Symbol:   {}", symbol);
    println!("  Decimals: {}", cfg.decimals);
    println!("  Tier:     {}", tier);
    if let Some(cap) = cfg.supply_cap {
        println!("  Supply Cap: {}", cap);
    }

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let system_program = Pubkey::from_str(SYSTEM_PROGRAM)?;
    let token_program = Pubkey::from_str(&signer::get_token_2022_program_id())?;

    let mint = Keypair::new();
    let authority = keypair.pubkey();

    let (config_pda, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint.pubkey().to_bytes()],
        &program_id,
    );

    // Space for mint with PermanentDelegate extension
    let mint_space = spl_token_2022::extension::ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(
        &[spl_token_2022::extension::ExtensionType::PermanentDelegate]
    ).unwrap();
    let lamports = rpc.get_minimum_balance_for_rent_exemption(mint_space)?;

    // Ix 1: allocate mint account
    let create_mint_ix = solana_sdk::system_instruction::create_account(
        &authority,
        &mint.pubkey(),
        lamports,
        mint_space as u64,
        &token_program,
    );

    // Ix 2: set permanent delegate to config PDA (must be before initialize_mint2)
    let init_permanent_delegate_ix =
        spl_token_2022::instruction::initialize_permanent_delegate(
            &token_program,
            &mint.pubkey(),
            &config_pda,
        )?;

    // Ix 3: initialize mint
    let initialize_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &token_program,
        &mint.pubkey(),
        &authority,
        Some(&authority),
        cfg.decimals,
    )?;

    // Ix 4: program initialize — no preset byte in new architecture
    let disc = compute_discriminator("global:initialize");
    let mut data = disc.to_vec();
    // supply_cap: Option<u64>
    if let Some(cap) = cfg.supply_cap {
        data.push(1);
        data.extend_from_slice(&cap.to_le_bytes());
    } else {
        data.push(0);
    }
    data.push(cfg.decimals);

    let init_ix = Instruction::new_with_bytes(
        program_id,
        &data,
        vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
    );

    let tx = Transaction::new_with_payer(
        &[create_mint_ix, init_permanent_delegate_ix, initialize_mint_ix, init_ix],
        Some(&authority),
    );

    let signature = rpc
        .send_transaction(tx, &[keypair as &dyn Signer, &mint as &dyn Signer])
        .await?;

    let mint_address = mint.pubkey().to_string();
    let config_address = config_pda.to_string();

    // Save mint to config.toml
    let cli_cfg = crate::config::CliConfig::load().unwrap_or_default();
    if let Err(e) = cli_cfg.save_mint(&mint_address) {
        eprintln!("Note: Could not save mint to config: {}", e);
    } else {
        println!("\n✅ Mint address saved to config.toml");
    }

    println!("\n✅ Stablecoin initialized ({})!", tier);
    println!("  Mint:      {}", mint_address);
    println!("  Config:    {}", config_address);
    println!("  Signature: {}", signature);
    println!("  Solscan:   https://solscan.io/tx/{}?cluster=devnet", signature);

    // Optionally attach compliance module (SSS-2)
    if let Some(ref blacklister_addr) = blacklister {
        println!("\n🔒 Attaching compliance module (blacklister: {})...", blacklister_addr);
        crate::commands::compliance::attach(rpc, keypair, blacklister_addr, Some(mint_address.clone())).await?;
    }

    // Optionally attach privacy module (SSS-3)
    if let Some(ref allowlist_auth) = allowlist_authority {
        println!("\n🔐 Attaching privacy module (allowlist authority: {})...", allowlist_auth);
        crate::commands::privacy::attach(rpc, keypair, allowlist_auth, false, Some(mint_address.clone())).await?;
    }

    Ok(())
}
