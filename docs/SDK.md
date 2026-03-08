# SDK Documentation

The SSS TypeScript SDK provides a convenient interface for interacting with the Solana Stablecoin Standard program from JavaScript/TypeScript applications.

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Presets](#presets)
- [Configuration Options](#configuration-options)
- [API Reference](#api-reference)
- [Examples](#examples)

---

## Installation

```bash
npm install @stbr/sss-token
```

Or build from source:

```bash
just sdk-build
```

---

## Quick Start

```typescript
import { SolanaStablecoin, Presets } from "@stbr/sss-token";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const authority = Keypair.fromSecretKey(/* your keypair */);

// Create a new SSS-2 compliant stablecoin
const stablecoin = await SolanaStablecoin.create(connection, {
  preset: Presets.SSS_2,
  name: "USD Coin",
  symbol: "USDC",
  decimals: 6,
  authority: authority,
});

// Mint tokens to a recipient
await stablecoin.mint({
  recipient: "recipient-wallet-address",
  amount: 1_000_000, // 1 USDC (6 decimals)
  minter: authority,
});
```

---

## Presets

The SDK supports two presets that map to the tier system:

```typescript
export const Presets = {
  SSS_1: 0, // Non-compliant mode
  SSS_2: 1, // Compliant mode
} as const;

export const PresetNames = {
  0: "SSS_1",
  1: "SSS_2",
} as const;

export const PresetDescriptions: Record<number, string> = {
  0: "Non-compliant mode - basic stablecoin operations",
  1: "Compliant mode - blacklist, freeze, seize, transfer hook",
};
```

| Preset  | Value | Description                                           |
| ------- | ----- | ----------------------------------------------------- |
| `SSS_1` | 0     | Base stablecoin - mint, burn, freeze/thaw, pause      |
| `SSS_2` | 1     | Compliant - SSS-1 + blacklist, seizure, transfer hook |

---

## Configuration Options

### Create Options

```typescript
interface CreateOptions {
  /** The compliance preset (SSS_1 or SSS_2) */
  preset: number;

  /** Token name (e.g., "USD Coin") */
  name: string;

  /** Token symbol (e.g., "USDC") */
  symbol: string;

  /** Token decimals (typically 6 for stablecoins) */
  decimals: number;

  /** The authority keypair for initialization */
  authority: Keypair;

  /** Optional: Maximum supply cap */
  supplyCap?: number;

  /** Optional: Custom program ID */
  programId?: PublicKey;
}
```

### Fetch Options

```typescript
interface FetchOptions {
  /** Connection to Solana */
  connection: Connection;

  /** The mint address */
  mint: PublicKey;

  /** Optional: Custom program ID */
  programId?: PublicKey;
}
```

---

## API Reference

### SolanaStablecoin.create()

Creates a new stablecoin mint and initializes the config.

```typescript
static async create(
  connection: Connection,
  options: CreateOptions
): Promise<SolanaStablecoin>
```

### SolanaStablecoin.fetch()

Fetches an existing stablecoin by mint address.

```typescript
static async fetch(
  connection: Connection,
  mint: PublicKey,
  options?: { programId?: PublicKey }
): Promise<SolanaStablecoin>
```

### Instance Methods

#### Core Operations

```typescript
// Mint new tokens
await stablecoin.mint({
  recipient: string,    // Recipient wallet address
  amount: number,      // Amount in smallest units
  minter: Keypair,      // Minter authority
});

// Burn tokens
await stablecoin.burn({
  account: string,      // Token account to burn from
  amount: number,      // Amount in smallest units
  authority: Keypair,  // Burn authority
});

// Transfer tokens
await stablecoin.transfer({
  from: string,        // Source token account
  to: string,          // Destination token account
  amount: number,      // Amount in smallest units
  authority: Keypair,  // Transfer authority
});

// Freeze an account
await stablecoin.freeze({
  account: string,     // Token account to freeze
  authority: Keypair, // Freezer authority
});

// Thaw a frozen account
await stablecoin.thaw({
  account: string,     // Token account to thaw
  authority: Keypair, // Freezer authority
});

// Pause all transfers
await stablecoin.pause(authority: Keypair);

// Unpause transfers
await stablecoin.unpause(authority: Keypair);
```

#### Compliance (SSS-2 Only)

```typescript
// Access compliance module
const compliance = stablecoin.compliance;

// Add to blacklist
await compliance.blacklistAdd({
  address: string, // Address to blacklist
  reason: string, // Reason (max 128 chars)
  blacklister: Keypair,
});

// Remove from blacklist
await compliance.blacklistRemove({
  address: string, // Address to remove
  authority: Keypair,
});

// Seize tokens from blacklisted account
await compliance.seize({
  from: string, // Source (blacklisted) account
  to: string, // Destination account
  amount: number, // Amount to seize
  authority: Keypair, // Seizer authority
});

// Freeze account
await compliance.freeze({
  account: string,
  authority: Keypair,
});

// Thaw account
await compliance.thaw({
  account: string,
  authority: Keypair,
});
```

#### Privacy (SSS-3 Only)

```typescript
// Access privacy module
const privacy = stablecoin.privacy;

// Add to allowlist
await privacy.allowlistAdd({
  wallet: string, // Wallet to whitelist
  allowlistAuthority: Keypair,
});

// Remove from allowlist
await privacy.allowlistRemove({
  wallet: string,
  authority: Keypair,
});
```

#### Query Methods

```typescript
// Get total supply
const supply = await stablecoin.getTotalSupply();

// Get config info
const config = await stablecoin.getConfig();

// Get mint info
const mintInfo = await stablecoin.getMintInfo();

// Check if paused
const isPaused = await stablecoin.isPaused();

// Get tier
const tier = stablecoin.getTier(); // 'SSS-1' | 'SSS-2' | 'SSS-3'
```

---

## Examples

### SSS-1: Basic Stablecoin

```typescript
import { SolanaStablecoin, Presets } from "@stbr/sss-token";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const authority = Keypair.fromSecretKey(/* your keypair */);

// Create SSS-1 stablecoin
const stablecoin = await SolanaStablecoin.create(connection, {
  preset: Presets.SSS_1,
  name: "Test USD",
  symbol: "TUSD",
  decimals: 6,
  authority: authority,
});

// Basic operations
await stablecoin.mint({
  recipient: "recipient-address",
  amount: 1_000_000,
  minter: authority,
});

await stablecoin.transfer({
  from: "sender-token-account",
  to: "recipient-token-account",
  amount: 100_000,
  authority: authority,
});

await stablecoin.pause(authority);

// Query
const supply = await stablecoin.getTotalSupply();
console.log(`Total supply: ${supply}`);
```

### SSS-2: Compliant Stablecoin

```typescript
import { SolanaStablecoin, Presets } from "@stbr/sss-token";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const authority = Keypair.fromSecretKey(/* your keypair */);

// Create SSS-2 compliant stablecoin
const stablecoin = await SolanaStablecoin.create(connection, {
  preset: Presets.SSS_2,
  name: "Regulated USD",
  symbol: "RUSD",
  decimals: 6,
  authority: authority,
});

// Mint with compliance
await stablecoin.mint({
  recipient: "recipient-address",
  amount: 1_000_000,
  minter: authority,
});

// Blacklist a suspicious address
await stablecoin.compliance.blacklistAdd({
  address: "suspicious-address",
  reason: "Suspected fraud",
  blacklister: authority,
});

// Check if transfer is allowed
const compliance = stablecoin.compliance;
const checkResult = await compliance.checkAddress("user-address");
if (!checkResult.allowed) {
  console.log(`Transfer blocked: ${checkResult.reason}`);
}

// Seize funds from blacklisted account
await stablecoin.compliance.seize({
  from: "blacklisted-account",
  to: "treasury-account",
  amount: 500_000,
  authority: authority,
});

// Freeze suspicious account
await stablecoin.compliance.freeze({
  account: "suspicious-account",
  authority: authority,
});
```

### SSS-3: Privacy-Enabled Stablecoin

```typescript
import { SolanaStablecoin, Presets } from "@stbr/sss-token";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const authority = Keypair.fromSecretKey(/* your keypair */);

// Create SSS-3 stablecoin (requires SSS-2 first, then attach privacy)
const stablecoin = await SolanaStablecoin.create(connection, {
  preset: Presets.SSS_2,
  name: "Private USD",
  symbol: "PUSD",
  decimals: 6,
  authority: authority,
});

// Attach privacy module
await stablecoin.attachPrivacy({
  allowlistAuthority: authority,
});

// Add trusted addresses to allowlist
await stablecoin.privacy.allowlistAdd({
  wallet: "trusted-partner-address",
  allowlistAuthority: authority,
});

// Remove from allowlist
await stablecoin.privacy.allowlistRemove({
  wallet: "trusted-partner-address",
  authority: authority,
});
```

### Fetching Existing Stablecoin

```typescript
import { SolanaStablecoin } from "@stbr/sss-token";
import { Connection, PublicKey } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const mintAddress = new PublicKey("YourMintAddress...");

// Fetch existing stablecoin
const stablecoin = await SolanaStablecoin.fetch(connection, mintAddress);

// Get info
console.log(`Name: ${stablecoin.name}`);
console.log(`Symbol: ${stablecoin.symbol}`);
console.log(`Decimals: ${stablecoin.decimals}`);
console.log(`Tier: ${stablecoin.getTier()}`);

// Get supply
const supply = await stablecoin.getTotalSupply();

// Get config
const config = await stablecoin.getConfig();
```
