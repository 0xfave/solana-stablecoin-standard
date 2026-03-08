# Architecture

This document describes the Solana Stablecoin Standard (SSS) architecture, including the layer model, data flows, and security modes.

---

## Table of Contents

- [Layer Model](#layer-model)
- [Data Flows](#data-flows)
- [Security Modes](#security-modes)
- [Account Structure](#account-structure)
- [PDA Derivation](#pda-derivation)
- [Event System](#event-system)

---

## Layer Model

SSS follows a layered architecture that separates concerns and enables modular upgrades:

```mermaid
graph TB
    subgraph Client["Client Layer"]
        CLI[CLI / TUI]
        SDK[TypeScript SDK]
        Frontend[Frontend UI]
        Backend[Backend API]
    end

    subgraph Program["On-Chain Program Layer"]
        SSS[SSS Program<br/>C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw]
        Hook[Compliance Hook<br/>sss_compliance_hook]
    end

    subgraph Token["Token Layer"]
        T2022[Token-2022]
        Mint[Mint Account]
    end

    subgraph Module["Module Layer"]
        Config[StablecoinConfig]
        Compliance[ComplianceModule]
        Privacy[PrivacyModule]
    end

    Client -->|"Instructions"| Program
    SSS -->|"CPI"| Token
    Hook -->|"Validates transfers"| Token
    Config -->|"Controls"| Mint
    Config -->|"Attachable"| Compliance
    Config -->|"Attachable"| Privacy
```

### Layer Responsibilities

| Layer       | Responsibility   | Components                      |
| ----------- | ---------------- | ------------------------------- |
| **Client**  | User interaction | CLI, SDK, Frontend, Backend     |
| **Program** | Business logic   | SSS Program, Compliance Hook    |
| **Token**   | Token operations | Token-2022, Mint, TokenAccounts |
| **Module**  | Feature control  | Config, Compliance, Privacy     |

---

## Data Flows

### Mint Flow (Fiat On-Ramp)

```mermaid
sequenceDiagram
    participant U as User
    participant C as Custodian
    participant API as Backend
    participant S as Solana
    participant B as Backend

    C->>C: Fiat received
    C->>API: Notify fiat confirmed
    API->>S: Submit mint tx
    S-->>API: Return signature
    API->>B: Store mint request
    loop Poll confirmation
        API->>S: Check signature
    end
    S-->>API: Confirmed
    API->>U: Notify complete
```

### Transfer Flow (SSS-2/SSS-3)

```mermaid
sequenceDiagram
    participant S as Sender
    participant R as Receiver
    participant T as Token-2022
    participant H as Compliance Hook
    participant B as Blacklist
    participant A as Allowlist

    S->>T: Transfer instruction
    T->>H: Execute transfer hook (before)
    H->>B: Check sender not blacklisted
    H->>B: Check receiver not blacklisted
    H->>A: Check receiver in allowlist (SSS-3 only)
    alt Any check fails
        H-->>T: Reject transfer
        T-->>S: Failed
    else All checks pass
        H-->>T: Approved
        T->>T: Execute transfer
        T->>H: Execute transfer hook (after)
        H-->>S: Success
    end
```

### Seize Flow (SSS-2 Only)

```mermaid
sequenceDiagram
    participant A as Authority
    participant S as SSS Program
    participant M as Mint
    participant T as Token-2022
    participant Src as Source Account
    participant Dst as Destination

    A->>S: Seize instruction
    S->>M: Verify permanent delegate
    S->>T: CPI transfer_checked
    note right of T: Permanent delegate<br/>authorizes transfer
    T->>Dst: Tokens transferred
    S->>S: Emit TokensSeized event
```

### Module Attach/Detach Flow

```mermaid
sequenceDiagram
    participant M as Master Authority
    participant C as StablecoinConfig
    participant CM as ComplianceModule
    participant PM as PrivacyModule

    Note over M,PM: Attach Compliance Module
    M->>C: Attach compliance
    C->>CM: Initialize PDA
    CM->>C: Store reference
    Note over C: Tier = SSS-2

    Note over M,PM: Attach Privacy Module
    M->>C: Attach privacy
    C->>PM: Initialize PDA
    PM->>C: Store reference
    Note over C: Tier = SSS-3

    Note over M,PM: Detach Modules (Downgrade)
    M->>C: Detach compliance
    C->>CM: Close account
    M->>C: Detach privacy
    C->>PM: Close account
    Note over C: Tier = SSS-1
```

---

## Security Modes

### Mode 1: SSS-1 (Base)

**Characteristics:**

- No compliance module attached
- No privacy module attached
- Basic token operations only

**Security Model:**

- Single authority (master_authority)
- Role-based permissions (minter, freezer, pauser)
- Supply cap enforcement
- Pause functionality

**Threats Mitigated:**

- Unauthorized minting (minter role)
- Unauthorized freezing (freezer role)
- Supply overflow (supply cap)

### Mode 2: SSS-2 (Compliance)

**Adds:**

- ComplianceModule with blacklister role
- Transfer hook for compliance enforcement
- Permanent delegate for seizure

**Security Model:**

- All SSS-1 controls
- Blacklist enforcement on every transfer
- Seizure capability for sanctioned accounts
- Audit trail for all compliance actions

**Additional Threats Mitigated:**

- Sanctioned entity transfers
- Fraudulent account activity
- Regulatory non-compliance

### Mode 3: SSS-3 (Privacy)

**Adds:**

- PrivacyModule with allowlist authority
- Allowlist gating for transfers
- Confidential transfers support

**Security Model:**

- All SSS-2 controls
- Transfer allowlisting
- Optional confidential transfers

**Additional Threats Mitigated:**

- Unauthorized recipient transfers
- Privacy breach attempts

---

## Account Structure

### StablecoinConfig

```rust
pub struct StablecoinConfig {
    pub master_authority: Pubkey,      // 32 - full control
    pub mint: Pubkey,                  // 32 - associated mint
    pub paused: bool,                  // 1  - global pause
    pub supply_cap: Option<u64>,       // 9  - max supply
    pub decimals: u8,                  // 1  - token decimals
    pub bump: u8,                      // 1  - PDA bump
    pub pending_master_authority: Option<Pubkey>, // 33 - transfer pending
    pub minters: Vec<Pubkey>,          // 324 - authorized minters
    pub freezer: Pubkey,               // 32 - freeze authority
    pub pauser: Pubkey,                // 32 - pause authority
}
```

### ComplianceModule (SSS-2)

```rust
pub struct ComplianceModule {
    pub config: Pubkey,                 // 32 - back-ref to config
    pub authority: Pubkey,              // 32 - module authority
    pub blacklister: Pubkey,            // 32 - blacklist authority
    pub transfer_hook_program: Option<Pubkey>, // 33 - hook program
    pub permanent_delegate: Option<Pubkey>,    // 33 - seizure authority
    pub bump: u8,                       // 1
}
```

### PrivacyModule (SSS-3)

```rust
pub struct PrivacyModule {
    pub config: Pubkey,                 // 32 - back-ref to config
    pub authority: Pubkey,              // 32 - module authority
    pub allowlist_authority: Pubkey,    // 32 - allowlist authority
    pub confidential_transfers_enabled: bool, // 1
    pub bump: u8,                      // 1
}
```

### BlacklistEntry (SSS-2)

```rust
pub struct BlacklistEntry {
    pub blacklister: Pubkey,  // 32 - who added this entry
    pub reason: String,       // 128 - reason for blacklisting
    pub timestamp: i64,       // 8 - when added
    pub bump: u8,            // 1
}
```

### AllowlistEntry (SSS-3)

```rust
pub struct AllowlistEntry {
    pub wallet: Pubkey,      // 32 - whitelisted wallet
    pub approved_by: Pubkey, // 32 - who added
    pub approved_at: i64,   // 8 - when added
    pub bump: u8,           // 1
}
```

---

## PDA Derivation

| Account          | Seeds                                    | Authority |
| ---------------- | ---------------------------------------- | --------- |
| StablecoinConfig | `[b"stablecoin", mint]`                  | Program   |
| ComplianceModule | `[b"compliance", config]`                | Program   |
| PrivacyModule    | `[b"privacy", config]`                   | Program   |
| BlacklistEntry   | `[b"blacklist", config, target]`         | Program   |
| AllowlistEntry   | `[b"allowlist", privacy_module, wallet]` | Program   |

---

## Event System

All significant actions emit on-chain events:

```rust
enum Event {
    ConfigInitialized { mint: Pubkey, authority: Pubkey },
    TokensMinted { amount: u64, recipient: Pubkey },
    TokensBurned { amount: u64, account: Pubkey },
    AccountFrozen { account: Pubkey },
    AccountThawed { account: Pubkey },
    PausedChanged { paused: bool },
    AddedToBlacklist { address: Pubkey, reason: String },
    RemovedFromBlacklist { address: Pubkey },
    TokensSeized { from: Pubkey, to: Pubkey, amount: u64 },
    MinterUpdated { added: bool, minter: Pubkey },
    FreezerUpdated { freezer: Pubkey },
    PauserUpdated { pauser: Pubkey },
    BlacklisterUpdated { blacklister: Pubkey },
}
```

Events are indexed by the backend for queryable access via the API.
