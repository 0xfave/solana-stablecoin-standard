use anyhow::Result;
use clap::{Parser, Subcommand};
use solana_sdk::signature::Signer;

mod commands;
mod config;
mod rpc_client;
mod signer;
mod tui;

use commands::blacklist::BlacklistAction;
use commands::minters::MinterAction;
use commands::*;

#[derive(Parser)]
#[command(name = "sss-token")]
#[command(about = "CLI for Solana Stablecoin Standard", long_about = None)]
struct Cli {
    /// RPC endpoint
    #[arg(short, long)]
    rpc: Option<String>,

    /// Keypair path
    #[arg(short, long)]
    keypair: Option<String>,

    /// Mint address
    #[arg(short, long)]
    mint: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        /// Preset: sss-1, sss-2
        #[arg(long)]
        preset: Option<String>,

        /// Token name (e.g., "My USD Coin")
        #[arg(long)]
        name: Option<String>,

        /// Token symbol (e.g., "USDC")
        #[arg(long)]
        symbol: Option<String>,

        /// Supply cap
        #[arg(long)]
        supply_cap: Option<u64>,

        /// Custom config file (TOML/JSON)
        #[arg(long)]
        config: Option<String>,
    },
    Mint {
        recipient: String,
        amount: u64,
    },
    Burn {
        amount: u64,
    },
    Freeze {
        address: String,
    },
    Thaw {
        address: String,
    },
    Pause,
    Unpause,
    Status,
    Supply,
    Blacklist {
        #[command(subcommand)]
        action: BlacklistAction,
    },
    Seize {
        address: String,
        #[arg(long, short)]
        to: String,
        amount: u64,
    },
    Minters {
        #[command(subcommand)]
        action: MinterAction,
    },
    Holders {
        #[arg(long, short)]
        min_balance: Option<u64>,
    },
    AuditLog {
        #[arg(long, short)]
        action: Option<String>,
    },
    
    Tui,
}

fn load_config() -> config::CliConfig {
    match config::CliConfig::load() {
        Ok(c) => c,
        Err(_) => config::CliConfig::default(),
    }
}

fn get_mint(cli: &Cli, cfg: &config::CliConfig) -> Option<String> {
    cli.mint.clone().or(cfg.get_mint())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_config();

    let rpc = cli.rpc.clone().unwrap_or_else(|| cfg.get_rpc());
    let rpc_client = rpc_client::RpcClient::new(rpc.clone());
    
    let keypair_path = cli.keypair.clone().or(cfg.get_keypair());
    let keypair = match signer::load_keypair(keypair_path.as_deref()) {
        Ok(kp) => kp,
        Err(e) => {
            eprintln!("\n❌ Error loading keypair: {}", e);
            eprintln!("\n📋 Setup options:\n");
            eprintln!("  Option 1: Create config.toml:");
            eprintln!("    [keypair]");
            eprintln!("    path = \"/path/to/keypair.json\"");
            eprintln!("");
            eprintln!("  Option 2: Use --keypair flag:");
            eprintln!("    sss-token --keypair /path/to/keypair.json <command>");
            eprintln!("");
            eprintln!("  Option 3: Default location:");
            eprintln!("    ~/.config/solana/id.json");
            std::process::exit(1);
        }
    };

    println!("👤 Wallet: {}", keypair.pubkey());
    println!("🌐 RPC:    {}", rpc);

    let mint = get_mint(&cli, &cfg);
    if let Some(ref m) = mint {
        println!("🏦 Mint:   {}\n", m);
    } else if !matches!(cli.command, Commands::Init { .. }) {
        println!("🏦 Mint:   (not set - run 'init' first)\n");
    } else {
        println!();
    }

    match cli.command {
        Commands::Init { preset, name, symbol, supply_cap, config } => {
            init::execute(&rpc_client, &keypair, preset, name, symbol, supply_cap, config).await?;
        }
        Commands::Mint { recipient, amount } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            mint::execute(&rpc_client, &keypair, &recipient, amount, Some(mint)).await?;
        }
        Commands::Burn { amount } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            burn::execute(&rpc_client, &keypair, amount, Some(mint)).await?;
        }
        Commands::Freeze { address } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            freeze::execute(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::Thaw { address } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            thaw::execute(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::Pause => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            pause::execute(&rpc_client, &keypair, true, Some(mint)).await?;
        }
        Commands::Unpause => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            pause::execute(&rpc_client, &keypair, false, Some(mint)).await?;
        }
        Commands::Status => {
            status::execute(&rpc_client, mint).await?;
        }
        Commands::Supply => {
            supply::execute(&rpc_client, mint).await?;
        }
        Commands::Blacklist { action } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            blacklist::execute(&rpc_client, &keypair, action, Some(mint)).await?;
        }
        Commands::Seize { address, to, amount } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            seize::execute(&rpc_client, &keypair, &address, &to, amount, Some(mint)).await?;
        }
        Commands::Minters { action } => {
            let mint = mint.ok_or_else(|| {
                anyhow::anyhow!("Mint not set. Add 'mint' to config.toml or run 'init' first.")
            })?;
            minters::execute(&rpc_client, &keypair, action, Some(mint)).await?;
        }
        Commands::Holders { min_balance } => {
            holders::execute(&rpc_client, mint, min_balance).await?;
        }
        Commands::AuditLog { action } => {
            audit_log::execute(action).await?;
        }
        Commands::Tui => {
            tui::run(&rpc_client, &keypair, mint).await?;
        }
    }

    Ok(())
}
