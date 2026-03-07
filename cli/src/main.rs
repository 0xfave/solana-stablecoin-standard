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
    /// Create a new SSS-1 token. Use --blacklister to attach compliance (SSS-2),
    /// or --allowlist-authority to attach privacy (SSS-3).
    Init {
        /// Token name (e.g., "My USD Coin")
        #[arg(long)]
        name: Option<String>,

        /// Token symbol (e.g., "USDC")
        #[arg(long)]
        symbol: Option<String>,

        /// Decimals (default: 6)
        #[arg(long)]
        decimals: Option<u8>,

        /// Supply cap in smallest units
        #[arg(long)]
        supply_cap: Option<u64>,

        /// Attach compliance module — sets this address as blacklister (upgrades to SSS-2)
        #[arg(long)]
        blacklister: Option<String>,

        /// Attach privacy module — sets this address as allowlist authority (upgrades to SSS-3)
        #[arg(long)]
        allowlist_authority: Option<String>,

        /// Custom config file (TOML/JSON)
        #[arg(long)]
        config: Option<String>,
    },

    /// Mint tokens to a recipient
    Mint {
        recipient: String,
        amount: u64,
    },

    /// Burn tokens from your ATA
    Burn {
        amount: u64,
    },

    /// Freeze a token account
    Freeze {
        address: String,
    },

    /// Thaw a frozen token account
    Thaw {
        address: String,
    },

    /// Pause all transfers
    Pause,

    /// Unpause transfers
    Unpause,

    /// Show token status
    Status,

    /// Show total supply
    Supply,

    /// Blacklist management (requires compliance module)
    Blacklist {
        #[command(subcommand)]
        action: BlacklistAction,
    },

    /// Seize tokens from a blacklisted address (requires compliance module)
    Seize {
        address: String,
        #[arg(long, short)]
        to: String,
        amount: u64,
    },

    /// Minter management
    Minters {
        #[command(subcommand)]
        action: MinterAction,
    },

    /// Attach the compliance module to an SSS-1 token (upgrades to SSS-2)
    AttachCompliance {
        /// Address that will be allowed to blacklist wallets
        #[arg(long)]
        blacklister: String,
    },

    /// Detach the compliance module (downgrades to SSS-1)
    DetachCompliance,

    /// Attach the privacy module (upgrades to SSS-3)
    AttachPrivacy {
        /// Address that will manage the allowlist
        #[arg(long)]
        allowlist_authority: String,

        /// Enable confidential transfers
        #[arg(long, default_value_t = false)]
        confidential: bool,
    },

    /// Detach the privacy module
    DetachPrivacy,

    /// Add an address to the privacy allowlist (requires privacy module)
    AllowlistAdd {
        address: String,
    },

    /// Remove an address from the privacy allowlist (requires privacy module)
    AllowlistRemove {
        address: String,
    },

    /// List all token holders
    Holders {
        #[arg(long, short)]
        min_balance: Option<u64>,
    },

    /// Launch the TUI
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
            eprintln!("    keypair = \"/path/to/keypair.json\"");
            eprintln!();
            eprintln!("  Option 2: Use --keypair flag:");
            eprintln!("    sss-token --keypair /path/to/keypair.json <command>");
            eprintln!();
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
        println!("🏦 Mint:   (not set — run 'init' first)\n");
    } else {
        println!();
    }

    match cli.command {
        Commands::Init { name, symbol, decimals, supply_cap, blacklister, allowlist_authority, config } => {
            init::execute(
                &rpc_client,
                &keypair,
                name,
                symbol,
                decimals,
                supply_cap,
                blacklister,
                allowlist_authority,
                config,
            )
            .await?;
        }
        Commands::Mint { recipient, amount } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            mint::execute(&rpc_client, &keypair, &recipient, amount, Some(mint)).await?;
        }
        Commands::Burn { amount } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            burn::execute(&rpc_client, &keypair, amount, Some(mint)).await?;
        }
        Commands::Freeze { address } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            freeze::execute(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::Thaw { address } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            thaw::execute(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::Pause => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            pause::execute(&rpc_client, &keypair, true, Some(mint)).await?;
        }
        Commands::Unpause => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            pause::execute(&rpc_client, &keypair, false, Some(mint)).await?;
        }
        Commands::Status => {
            status::execute(&rpc_client, mint).await?;
        }
        Commands::Supply => {
            supply::execute(&rpc_client, mint).await?;
        }
        Commands::Blacklist { action } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            blacklist::execute(&rpc_client, &keypair, action, Some(mint)).await?;
        }
        Commands::Seize { address, to, amount } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            seize::execute(&rpc_client, &keypair, &address, &to, amount, Some(mint)).await?;
        }
        Commands::Minters { action } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            minters::execute(&rpc_client, &keypair, action, Some(mint)).await?;
        }
        Commands::AttachCompliance { blacklister } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            compliance::attach(&rpc_client, &keypair, &blacklister, Some(mint)).await?;
        }
        Commands::DetachCompliance => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            compliance::detach(&rpc_client, &keypair, Some(mint)).await?;
        }
        Commands::AttachPrivacy { allowlist_authority, confidential } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            privacy::attach(&rpc_client, &keypair, &allowlist_authority, confidential, Some(mint)).await?;
        }
        Commands::DetachPrivacy => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            privacy::detach(&rpc_client, &keypair, Some(mint)).await?;
        }
        Commands::AllowlistAdd { address } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            privacy::allowlist_add(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::AllowlistRemove { address } => {
            let mint = mint.ok_or_else(|| anyhow::anyhow!("Mint not set. Run 'init' first."))?;
            privacy::allowlist_remove(&rpc_client, &keypair, &address, Some(mint)).await?;
        }
        Commands::Holders { min_balance } => {
            holders::execute(&rpc_client, mint, min_balance).await?;
        }
        Commands::Tui => {
            tui::run(&rpc_client, &keypair, mint).await?;
        }
    }

    Ok(())
}
