# Operations Runbook

This document provides operational procedures for managing a Solana Stablecoin Standard (SSS) deployment using the CLI and TUI tools.

---

## Table of Contents

- [Setup](#setup)
- [SSS-1 Operations](#sss-1-operations)
- [SSS-2 Operations](#sss-2-operations)
- [SSS-3 Operations](#sss-3-operations)
- [TUI Guide](#tui-guide)
- [Troubleshooting](#troubleshooting)

---

## Setup

### CLI Installation

```bash
cd cli
cargo build --release
# Binary: ./target/release/sss-cli
```

### Configuration

Create `cli/config.toml`:

```toml
rpc_url = "https://api.devnet.solana.com"
keypair_path = "~/.config/solana/id.json"
mint = "<your_mint_pubkey>"        # Set after initialization
program_id = "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw"
```

### Justfile Shortcuts

Many operations have shortcuts via `just`:

```bash
just cli-init                    # Initialize mint
just cli-mint <addr> <amt>       # Mint tokens
just cli-burn <addr> <amt>       # Burn tokens
just cli-freeze <account>        # Freeze account
just cli-thaw <account>          # Thaw account
just cli-pause                   # Pause transfers
just cli-unpause                 # Unpause transfers
just cli-attach-compliance <pk>  # Attach compliance
just cli-detach-compliance       # Detach compliance
just cli-blacklist-add <addr>    # Add to blacklist
just cli-blacklist-remove <addr> # Remove from blacklist
just cli-attach-privacy <pk>     # Attach privacy
just cli-detach-privacy          # Detach privacy
just cli-allowlist-add <addr>    # Add to allowlist
just cli-allowlist-remove <addr> # Remove from allowlist
```

---

## SSS-1 Operations

### Initialize a New Stablecoin

```bash
sss-cli init --supply-cap 1000000000000 --decimals 6
```

This creates:

- A new Token-2022 mint
- StablecoinConfig PDA
- Sets the config as permanent delegate

**Parameters:**

- `--supply-cap`: Maximum token supply (optional)
- `--decimals`: Token decimals (default: 6)

**Output:** Prints the mint address - copy to `config.toml`

### Mint Tokens

```bash
sss-cli mint --to <wallet> --amount <amount>
```

**Parameters:**

- `--to`: Recipient wallet address
- `--amount`: Amount in smallest units (e.g., 1000000 = 1 token at 6 decimals)

**Example:**

```bash
sss-cli mint --to Dh4zqB1Kp8x4K3vL5RWxX6Y8jN9pQr2sTu3vW4yZzA --amount 1000000
```

### Burn Tokens

```bash
sss-cli burn --from <token_account> --amount <amount>
```

**Parameters:**

- `--from`: Token account to burn from
- `--amount`: Amount to burn

### Freeze Account

Freezes a token account, preventing transfers.

```bash
sss-cli freeze --account <token_account>
```

**Parameters:**

- `--account`: Token account to freeze

### Thaw Account

Unfreezes a previously frozen token account.

```bash
sss-cli thaw --account <token_account>
```

**Parameters:**

- `--account`: Token account to thaw

### Pause / Unpause

Pause all token transfers globally:

```bash
sss-cli pause
sss-cli unpause
```

### Minter Management

Add or remove minters:

```bash
# Add minter (requires master authority)
sss-cli add-minter --address <pubkey>

# Remove minter
sss-cli remove-minter --address <pubkey>
```

---

## SSS-2 Operations

### Attach Compliance Module

Before using compliance features, attach the compliance module:

```bash
sss-cli attach-compliance --blacklister <pubkey>
```

**Parameters:**

- `--blacklister`: Pubkey authorized to manage blacklist

This enables:

- Blacklist functionality
- Seizure capability
- Transfer hook enforcement

### Detach Compliance Module

Downgrade to SSS-1:

```bash
sss-cli detach-compliance
```

**Warning:** This removes all blacklist entries and compliance functionality.

### Blacklist Management

#### Add to Blacklist

```bash
sss-cli blacklist add <wallet> --reason "Sanctions"
```

**Parameters:**

- `<wallet>`: Address to blacklist
- `--reason`: Reason for blacklisting (max 128 chars)

**Example:**

```bash
sss-cli blacklist add 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU --reason "OFAC sanctions"
```

#### Remove from Blacklist

```bash
sss-cli blacklist remove <wallet>
```

**Parameters:**

- `<wallet>`: Address to remove from blacklist

### Seize Tokens

Transfer tokens from a blacklisted account to a destination:

```bash
sss-cli seize --from <blacklisted_account> --to <destination> --amount <amount>
```

**Parameters:**

- `--from`: Source token account (must be blacklisted)
- `--to`: Destination token account
- `--amount`: Amount to seize

**Example:**

```bash
sss-cli seize --from 7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU --to TreasuryAccount --amount 500000
```

### Compliance Check

Check an address against the blacklist:

```bash
sss-cli compliance check <address>
```

---

## SSS-3 Operations

### Attach Privacy Module

Before using privacy features, attach the privacy module:

```bash
sss-cli attach-privacy --allowlist-authority <pubkey>
```

**Parameters:**

- `--allowlist-authority`: Pubkey authorized to manage allowlist

This enables:

- Allowlist gating for transfers
- Optional confidential transfers

### Detach Privacy Module

Downgrade to SSS-2:

```bash
sss-cli detach-privacy
```

**Warning:** This removes all allowlist entries and privacy functionality.

### Allowlist Management

#### Add to Allowlist

```bash
sss-cli allowlist add <wallet>
```

**Parameters:**

- `<wallet>`: Address to add to allowlist

**Example:**

```bash
sss-cli allowlist add Dh4zqB1Kp8x4K3vL5RWxX6Y8jN9pQr2sTu3vW4yZzA
```

#### Remove from Allowlist

```bash
sss-cli allowlist remove <wallet>
```

**Parameters:**

- `<wallet>`: Address to remove from allowlist

---

## TUI Guide

Launch the interactive terminal dashboard:

```bash
sss-cli tui
# or
just tui
```

### Navigation

- **Arrow keys**: Navigate between sections
- **Enter**: Select / confirm
- **Esc**: Go back / cancel
- **Ctrl+C**: Exit

### Sections

#### Dashboard (Home)

- Shows current tier (SSS-1/SSS-2/SSS-3)
- Total supply
- Paused status
- Recent events

#### Init (First Run)

- Initialize new mint
- Configure supply cap and decimals

#### Mint

- Mint tokens to wallet
- View mint history

#### Burn

- Burn tokens from account
- View burn history

#### Freeze

- Freeze / thaw accounts

#### Pause

- Global pause / unpause

#### Compliance (SSS-2+)

- View blacklist
- Add / remove from blacklist
- Seize tokens

#### Privacy (SSS-3+)

- View allowlist
- Add / remove from allowlist

---

## Troubleshooting

### Transaction Fails with "Blacklisted"

The sender or receiver is on the blacklist. Check:

```bash
sss-cli compliance check <address>
```

### Transaction Fails with "Not on Allowlist"

In SSS-3, transfers require the receiver to be on the allowlist:

```bash
sss-cli allowlist add <address>
```

### Transaction Fails with "Account Frozen"

The account has been frozen. Thaw it:

```bash
sss-cli thaw --account <token_account>
```

### Transaction Fails with "Paused"

Global transfers are paused. Unpause:

```bash
sss-cli unpause
```

### Insufficient Funds for Transaction

Ensure your authority wallet has enough SOL for:

- Account creation (new token accounts)
- Transaction fees

### Invalid Mint Address

Copy the correct mint address to `config.toml` after initialization:

```toml
mint = "YourCorrectMintAddressHere"
```
