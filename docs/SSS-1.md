# SSS-1: Minimal Stablecoin Standard

This document defines the SSS-1 specification - the base tier of the Solana Stablecoin Standard. SSS-1 provides core stablecoin functionality without compliance features.

---

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Account Structure](#account-structure)
- [Instructions](#instructions)
- [PDA Reference](#pda-reference)
- [Security Model](#security-model)
- [Upgrading to SSS-2](#upgrading-to-sss-2)

---

## Overview

SSS-1 is the foundational tier providing:

- Token minting and burning
- Account freeze/thaw
- Global pause functionality
- Supply cap enforcement
- Role-based permissions

**Status:** Base tier - no modules attached

---

## Features

### Core Operations

| Feature     | Description                                |
| ----------- | ------------------------------------------ |
| **Mint**    | Create new tokens (requires minter role)   |
| **Burn**    | Destroy tokens (anyone with token account) |
| **Freeze**  | Freeze a specific token account            |
| **Thaw**    | Unfreeze a frozen account                  |
| **Pause**   | Stop all transfers globally                |
| **Unpause** | Resume transfers                           |

### Role Management

| Role                 | Capability                     |
| -------------------- | ------------------------------ |
| **Master Authority** | Full control, can assign roles |
| **Minter**           | Mint new tokens                |
| **Freezer**          | Freeze/thaw accounts           |
| **Pauser**           | Pause/unpause transfers        |

---

## Account Structure

### StablecoinConfig

```rust
pub struct StablecoinConfig {
    pub master_authority: Pubkey,           // 32 - full control
    pub mint: Pubkey,                       // 32 - associated mint
    pub paused: bool,                        // 1  - global pause flag
    pub supply_cap: Option<u64>,             // 9  - max supply (optional)
    pub decimals: u8,                        // 1  - token decimals
    pub bump: u8,                            // 1  - PDA bump
    pub pending_master_authority: Option<Pubkey>, // 33 - transfer pending
    pub minters: Vec<Pubkey>,                // 324 - authorized minters
    pub freezer: Pubkey,                     // 32 - freeze authority
    pub pauser: Pubkey,                      // 32 - pause authority
}
```

### Account Layout (Bytes)

| Offset | Size | Field                             |
| ------ | ---- | --------------------------------- |
| 0      | 32   | master_authority                  |
| 32     | 32   | mint                              |
| 64     | 1    | paused                            |
| 65     | 9    | supply_cap (Option)               |
| 74     | 1    | decimals                          |
| 75     | 1    | bump                              |
| 76     | 33   | pending_master_authority (Option) |
| 109    | 4    | minters (Vec length)              |
| 113    | 320  | minters (Vec data, max 10)        |
| 433    | 32   | freezer                           |
| 465    | 32   | pauser                            |

---

## Instructions

### initialize

Initialize a new stablecoin mint with Token-2022.

**Discriminator:** `afaf6d1f0d989bed`

**Arguments:**

```rust
struct InitializeArgs {
    pub supply_cap: Option<u64>,  // Maximum supply (None = unlimited)
    pub decimals: u8,             // Token decimals
}
```

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | ✅ | ✅ | StablecoinConfig PDA (init) |
| mint | ✅ | ✅ | Token-2022 mint |
| master_authority | | ✅ | Initial authority |
| token_program | | | Token-2022 program |
| system_program | | | System program |

**Logic:**

1. Derive config PDA: `[b"stablecoin", mint]`
2. Initialize Token-2022 mint with extensions
3. Set config as permanent delegate
4. Store master_authority, mint, decimals, supply_cap
5. Set initial roles to master_authority
6. Emit `ConfigInitialized` event

---

### mint_tokens

Create new tokens.

**Discriminator:** `3b8418f627a708f3`

**Arguments:**

```rust
struct MintTokensArgs {
    pub amount: u64,  // Amount to mint
}
```

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | | | StablecoinConfig |
| mint | ✅ | | Token-2022 mint |
| destination | ✅ | | Token account to credit |
| minter | | ✅ | Minter authority |
| token_program | | | Token-2022 program |

**Logic:**

1. Verify minter is authorized
2. Check supply_cap not exceeded
3. Mint tokens to destination
4. Emit `TokensMinted` event

---

### burn_tokens

Destroy tokens.

**Discriminator:** `4c0f33fee5d77942`

**Arguments:**

```rust
struct BurnTokensArgs {
    pub amount: u64,  // Amount to burn
}
```

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | | | StablecoinConfig |
| mint | ✅ | | Token-2022 mint |
| from | ✅ | | Token account to burn from |
| burner | | ✅ | Authority |
| token_program | | | Token-2022 program |

**Logic:**
is1. Verify burner token owner or authorized 2. Burn tokens from account 3. Emit `TokensBurned` event

---

### freeze_account

Freeze a token account, preventing transfers.

**Discriminator:** `fd4b5285a7ee2b82`

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | | | StablecoinConfig |
| mint | | | Token-2022 mint |
| account | ✅ | | Token account to freeze |
| freezer | | ✅ | Freezer authority |
| token_program | | | Token-2022 program |

**Logic:**

1. Verify freezer is authorized
2. Freeze the token account
3. Emit `AccountFrozen` event

---

### thaw_account

Unfreeze a previously frozen token account.

**Discriminator:** `73984fd5d5a9b823`

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | | | StablecoinConfig |
| mint | | | Token-2022 mint |
| account | ✅ | | Token account to thaw |
| freezer | | ✅ | Freezer authority |
| token_program | | | Token-2022 program |

**Logic:**

1. Verify freezer is authorized
2. Thaw the token account
3. Emit `AccountThawed` event

---

### update_paused

Update global pause state.

**Discriminator:** `4eec5568a9e7cd59`

**Arguments:**

```rust
struct UpdatePausedArgs {
    pub paused: bool,  // true = pause, false = unpause
}
```

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | ✅ | | StablecoinConfig |
| pauser | | ✅ | Pauser authority |

**Logic:**

1. Verify pauser is authorized
2. Update paused flag
3. Emit `PausedChanged` event

---

### add_minter / remove_minter

Manage authorized minters.

**Discriminators:**

- add_minter: `4b56da28db068d1d`
- remove_minter: `f1455410a4e8834f`

**Accounts:**
| Role | Write | Sign | Description |
|------|-------|------|-------------|
| config | ✅ | | StablecoinConfig |
| authority | | ✅ | Master authority |

---

## PDA Reference

| Account          | Seeds                         | Size       |
| ---------------- | ----------------------------- | ---------- |
| StablecoinConfig | `[b"stablecoin", mint.key()]` | ~497 bytes |

---

## Security Model

### Authority Hierarchy

```
master_authority
    ├── minter (can mint)
    ├── freezer (can freeze/thaw)
    └── pauser (can pause/unpause)
```

### Access Control

- All critical operations require signer authority
- Config PDA is program-controlled (no private key)
- Roles can be separated for security

### Threat Mitigation

| Threat               | Mitigation                      |
| -------------------- | ------------------------------- |
| Unlimited minting    | supply_cap check                |
| Unauthorized minting | minter role verification        |
| Frozen funds stuck   | pauser can always pause/unpause |
| Supply overflow      | checked arithmetic              |

---

## Upgrading to SSS-2

To add compliance features, attach the compliance module:

```bash
sss-cli attach-compliance --blacklister <pubkey>
```

This creates a ComplianceModule PDA and enables:

- Blacklist functionality
- Transfer hook enforcement
- Token seizure capability

See [SSS-2 Specification](./SSS-2.md) for details.
