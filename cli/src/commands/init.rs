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

fn compute_discriminator(name: &str) -> [u8; 8] {
    use sha2::{Sha256, Digest};
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
    preset: Option<String>,
    name_arg: Option<String>,
    symbol_arg: Option<String>,
    supply_cap: Option<u64>,
    config: Option<String>,
) -> Result<()> {
    let cfg = if let Some(config_path) = config {
        StablecoinConfig::from_file(&config_path)?
    } else if let Some(preset_name) = preset {
        let c = StablecoinConfig::from_preset(&preset_name);
        c
    } else {
        StablecoinConfig::default()
    };

    // Prompt for name
    let name = if let Some(n) = name_arg {
        n
    } else {
        println!("\n📛 Enter token name (e.g., 'My USD Coin'): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let n = input.trim().to_string();
        if n.is_empty() {
            "My Stablecoin".to_string()
        } else {
            n
        }
    };

    // Prompt for symbol
    let symbol = if let Some(s) = symbol_arg {
        s
    } else {
        println!("\n💰 Enter token symbol (e.g., 'USDC'): ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let s = input.trim().to_string();
        if s.is_empty() {
            "STB".to_string()
        } else {
            s
        }
    };

    println!("\nInitializing stablecoin:");
    println!("  Name: {}", name);
    println!("  Symbol: {}", symbol);
    println!("  Decimals: {}", cfg.decimals);
    println!("  Preset: {}", cfg.preset);
    if let Some(cap) = cfg.supply_cap {
        println!("  Supply Cap: {}", cap);
    }

    let program_id = Pubkey::from_str(&signer::get_program_id())?;
    let system_program = Pubkey::from_str(&signer::get_system_program_id())?;
    let token_program = Pubkey::from_str(&signer::get_token_2022_program_id())?;

    let mint = Keypair::new();
    let authority = keypair.pubkey();

    let (config, _) = Pubkey::find_program_address(
        &[b"stablecoin", &mint.pubkey().to_bytes()],
        &program_id,
    );

    let preset_val = match cfg.preset.as_str() {
        "sss-1" | "sss_1" | "1" => 0u8,
        "sss-2" | "sss_2" | "2" => 1u8,
        _ => 0u8,
    };

    let mint_space = spl_token_2022::extension::ExtensionType::try_calculate_account_len::<spl_token_2022::state::Mint>(&[]).unwrap();
    let lamports = rpc.get_minimum_balance_for_rent_exemption(mint_space)?;

    let create_mint_ix = solana_sdk::system_instruction::create_account(
        &authority,
        &mint.pubkey(),
        lamports,
        mint_space as u64,
        &token_program,
    );

    let initialize_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &token_program,
        &mint.pubkey(),
        &authority,
        Some(&authority),
        cfg.decimals,
    )?;

    let disc = compute_discriminator("global:initialize");
    
    let mut data = disc.to_vec();
    data.push(preset_val);
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
            AccountMeta::new(config, false),
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(system_program, false),
        ],
    );

    let tx = Transaction::new_with_payer(&[create_mint_ix, initialize_mint_ix, init_ix], Some(&authority));

    let signature = rpc.send_transaction(tx, &[keypair as &dyn Signer, &mint as &dyn Signer]).await?;

    let mint_address = mint.pubkey().to_string();
    let config_address = config.to_string();
    
    if let Err(e) = crate::config::CliConfig::load() {
        eprintln!("Note: Could not save to config: {}", e);
    } else {
        let config = crate::config::CliConfig::load().unwrap_or_default();
        if let Err(e) = config.save_mint(&mint_address) {
            eprintln!("Note: Could not save mint to config: {}", e);
        } else {
            println!("\n✅ Mint address saved to config.toml");
        }
    }

    println!("\nStablecoin initialized successfully!");
    println!("  Mint: {}", mint_address);
    println!("  Config: {}", config_address);
    println!("  Signature: {}", signature);
    println!("  Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);

    Ok(())
}
