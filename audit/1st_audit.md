# Security Checklist — solana-stablecoin-standard

## Framework
Anchor · Token-2022 (via `token_interface`)

## Risk Level
🔴 Critical — Manages token minting/burning with admin key roles, blacklist/seizure mechanics, and potential large TVL. Multiple privileged roles (master_authority, minters, freezer, pauser, blacklister) and direct CPI chains to the token program.

---

## ⛔ CRITICAL VULNERABILITIES — Must Fix Before Deployment

### VULN-1 · Blacklist Bypass (Transfer Instruction)
**Severity: 🔴 Critical**

In `transfer()`, the blacklist check logic is:
```rust
if sender_blacklist_key == expected_sender_blacklist && *ctx.accounts.sender_blacklist.owner == ID {
    return Err(StablecoinError::SenderBlacklisted.into());
}
```
A blacklisted user can bypass this entirely by passing **any arbitrary account** as `sender_blacklist` whose key does not match `expected_sender_blacklist`. The condition is then `false`, the error never fires, and the transfer proceeds freely.

**Root cause:** The check requires _both_ conditions to match — but the accounts struct does not constrain which account is passed. An attacker chooses what to pass.

**Fix:** Require the provided account to be the correct PDA (enforce it in the constraint), then check whether it is initialized:
```rust
// In accounts struct:
#[account(
    seeds = [b"blacklist", config.key().as_ref(), from.owner.as_ref()],
    bump,
)]
pub sender_blacklist: Option<Account<'info, BlacklistEntry>>,

// In instruction:
require!(ctx.accounts.sender_blacklist.is_none(), StablecoinError::SenderBlacklisted);
```
Or keep as `UncheckedAccount` but derive and `require_keys_eq!` the PDA match before the owner check.

---

### VULN-2 · Unchecked Arithmetic Overflow (Mint Instruction)
**Severity: 🔴 Critical**

```rust
require!(ctx.accounts.mint.supply + amount <= cap, StablecoinError::Overflow);
```
Standard `+` on `u64` values will silently overflow in release builds (or panic in debug). If `supply + amount` wraps around past `u64::MAX`, the result is a tiny number that passes the cap check — allowing unlimited minting.

**Fix:**
```rust
let new_supply = ctx.accounts.mint.supply
    .checked_add(amount)
    .ok_or(StablecoinError::Overflow)?;
require!(new_supply <= cap, StablecoinError::Overflow);
```

---

### VULN-3 · Wrong Blacklister Authorization in `blacklist_add`
**Severity: 🔴 Critical**

```rust
require_keys_eq!(
    ctx.accounts.blacklister.key(),
    config.master_authority,   // ← should be config.blacklister
    StablecoinError::UnauthorizedBlacklister
);
```
The program has an `update_blacklister` instruction specifically to delegate blacklisting to a non-master key. But `blacklist_add` ignores `config.blacklister` entirely and only allows `master_authority` to blacklist. This makes the blacklister role non-functional — it can be updated but never actually used.

**Fix:**
```rust
require_keys_eq!(
    ctx.accounts.blacklister.key(),
    config.blacklister,
    StablecoinError::UnauthorizedBlacklister
);
```

---

## ⚠️ HIGH SEVERITY FINDINGS

### VULN-4 · `transfer()` Uses `token::transfer` Instead of `transfer_checked`
**Severity: 🟠 High**

The `transfer` instruction calls:
```rust
anchor_spl::token_interface::transfer(...)
```
This does NOT pass the `mint` account or `decimals`. For Token-2022 mints with transfer hooks or interest-bearing extensions, this can silently misbehave or fail to trigger mandatory extension logic. The `seize` instruction correctly uses `transfer_checked` — `transfer` should too.

**Fix:** Use `transfer_checked` with `mint` and `decimals`, matching the pattern in `seize`:
```rust
anchor_spl::token_interface::transfer_checked(
    CpiContext::new(...),
    amount,
    ctx.accounts.mint.decimals,
)?;
```
This also requires adding `mint` to the `Transfer` accounts struct.

---

### VULN-5 · Freeze Authority Never Initialized
**Severity: 🟠 High**

`initialize()` transfers mint authority to the config PDA:
```rust
AuthorityType::MintTokens → config PDA
```
But `freeze_account()` and `thaw_account()` CPI calls pass `ctx.accounts.freezer` (a `Signer`) as the authority. The freeze authority of the mint is **never set to the freezer** during initialization — it remains whatever was set when the mint was created (typically the original keypair, or None).

Unless the freeze authority is separately managed by the deployer, `freeze_account` and `thaw_account` will fail at runtime for any mint where the deployer's keypair no longer holds freeze authority.

**Fix options:**
1. During `initialize`, also transfer `AuthorityType::FreezeAccount` to the config PDA, and have `freeze_account`/`thaw_account` use PDA signing (like `mint`/`burn` do).
2. Or document explicitly that the mint's freeze authority must remain with `config.freezer` at deploy time and must be manually managed.

---

## 🟡 MEDIUM SEVERITY FINDINGS

### VULN-6 · No Duplicate Account Guard on `Transfer` and `Seize`
**Severity: 🟡 Medium**

In `Transfer`, both `from` and `to` are `mut InterfaceAccount<'info, TokenAccount>` with no constraint preventing them from being the same account. In `Seize`, `source` and `destination` have the same issue. Passing the same account for both roles causes conflicting writes; the last serialization wins.

**Fix — add to both account structs:**
```rust
// Transfer:
#[account(mut, constraint = from.key() != to.key() @ StablecoinError::SameAccount)]
pub from: InterfaceAccount<'info, TokenAccount>,

// Seize:
#[account(mut, constraint = source.key() != destination.key() @ StablecoinError::SameAccount)]
pub source: InterfaceAccount<'info, TokenAccount>,
```

---

### VULN-7 · `config.mint` Not Validated Against `mint` Account in Multiple Instructions
**Severity: 🟡 Medium**

In `MintTokens`, `Burn`, `FreezeAccount`, `ThawAccount`, and `Seize`, the `config` account is passed without any constraint linking it to the `mint` account. There is no `has_one = mint` or `constraint = config.mint == mint.key()`.

In practice the CPI would likely fail (since the config PDA is not the authority for an unrelated mint), but the missing constraint means the error surfaces deep in the token program rather than with a clear, early program error.

**Fix — add to all relevant account structs:**
```rust
pub config: Account<'info, StablecoinConfig>,  // existing
#[account(address = config.mint)]
pub mint: InterfaceAccount<'info, Mint>,
```
Or use `has_one = mint` if the field is named `mint` in `StablecoinConfig`.

---

### VULN-8 · `MintTokens` and `Burn` Account Structs Missing Seeds Constraint on `config`
**Severity: 🟡 Medium**

The `config` account in `MintTokens` and `Burn` is:
```rust
pub config: Account<'info, StablecoinConfig>,
```
No PDA seeds are enforced. Any valid `StablecoinConfig` account (owned by the program, correct discriminator) can be passed. The seeds constraint should be present to guarantee the config is the canonical one for that mint:
```rust
#[account(
    seeds = [b"stablecoin", mint.key().as_ref()],
    bump = config.bump
)]
pub config: Account<'info, StablecoinConfig>,
```

---

## 🔵 LOW / INFO FINDINGS

### INFO-1 · `burn()` Does Not Respect `paused` Flag
`mint()` checks `!config.paused` but `burn()` does not. Depending on protocol intent, burns during a pause may or may not be desirable — but the asymmetry is undocumented and likely unintentional. Add a check or add a comment explaining the design choice.

---

### INFO-2 · Minter Cap of 10 Is Silent and Undocumented
`require!(config.minters.len() < 10, ...)` — this hard cap is not reflected in any error message or documentation. Document it in the error enum and in any user-facing documentation.

---

### INFO-3 · No Timelock or Multisig on `master_authority`
`master_authority` controls all privileged operations. A single compromised keypair can add minters, seize funds, update transfer hooks, and blacklist users. For a 🔴 Critical protocol: a timelock or a multisig (e.g., Squads) for `master_authority` operations is strongly recommended before mainnet deployment.

---

### INFO-4 · `pending_master_authority` Is Declared But Never Used
`StablecoinConfig` has a `pending_master_authority: Option<Pubkey>` field, but there is no `propose_master_authority` or `accept_master_authority` instruction. Either implement 2-step authority transfer or remove the field to avoid dead state.

---

### INFO-5 · `initialize_extra_account_meta_list` Does Nothing
The instruction initializes nothing and emits no event — it only logs a message. If the intent is to set up a transfer hook's extra account meta list, this instruction is incomplete. If it is a placeholder, it should be gated or removed before mainnet.

---

### INFO-6 · `seize()` Validates Blacklist by `data_len() == 0` Check
```rust
if ctx.accounts.source_blacklist.data_len() == 0 {
    return Err(StablecoinError::NotBlacklisted.into());
}
```
Checking `data_len() == 0` is a fragile proxy for "account doesn't exist." A properly initialized zero-data account (or a closed but not yet garbage-collected account) could trip this incorrectly. The robust approach is to use `Account<'info, BlacklistEntry>` (with seeds + bump) or check the account discriminator explicitly.

---

## Rules Applied Table

| # | Category | Rule | Status | Notes |
|---|----------|------|--------|-------|
| 1 | Account Validation | Signer checks | ✅ | `Signer<'info>` used for all authority accounts |
| 2 | Account Validation | Ownership checks | ✅ | `Account<T>` used for typed state; `UncheckedAccount` documented with `/// CHECK:` |
| 3 | Account Validation | Cross-account relationships (has_one) | ❌ VULN-7 | `config.mint` not enforced against `mint` in MintTokens, Burn, FreezeAccount, ThawAccount, Seize |
| 4 | Account Validation | Type cosplay prevention | ✅ | Anchor discriminators on all state types |
| 5 | Account Validation | Reinitialization prevention | ✅ | `init` constraint used on config and blacklist entries |
| 6 | Account Validation | Writable flag enforcement | ✅ | `mut` only on accounts that are written |
| 7 | PDA Security | Canonical bump stored and reused | ✅ | `config.bump` stored at init, reused in all seed constraints |
| 8 | PDA Security | PDA sharing prevention | ✅ | Blacklist PDAs include `config` + `target` keys |
| 9 | PDA Security | Seed collision prevention | ✅ | `b"stablecoin"` and `b"blacklist"` are distinct prefixes |
| 10 | PDA Security | Seeds constraint on config in sensitive instructions | ❌ VULN-8 | MintTokens and Burn lack seeds constraint on config |
| 11 | Arithmetic | Checked math on financial values | ❌ VULN-2 | `supply + amount` uses unchecked `+` operator |
| 12 | Duplicate Accounts | Distinct mutable account guard | ❌ VULN-6 | Transfer and Seize allow from == to |
| 13 | CPI Safety | Program ID validation | ✅ | `Interface<'info, TokenInterface>` validates token program; `invoke_signed` uses program from accounts struct |
| 14 | CPI Safety | Error propagation with `?` | ✅ | All CPI calls use `?` |
| 15 | CPI Safety | Post-CPI reload | ✅ (minor) | No stale data used after CPIs in current logic |
| 16 | CPI Safety | invoke_signed scope minimized | ✅ | Only PDA accounts use invoke_signed |
| 17 | Token Safety | Token-2022 compatible transfer | ❌ VULN-4 | `transfer` instruction uses `token_interface::transfer` not `transfer_checked` |
| 18 | Token Safety | Mint account validated against config | ❌ VULN-7 | No `has_one = mint` in multiple instructions |
| 19 | Account Lifecycle | Safe account closing | ✅ | `close = destination` constraint used in `BlacklistRemove` |
| 20 | Account Lifecycle | Rent exemption | ✅ | All `init` accounts are payer-funded by Anchor |
| 21 | Business Logic | Blacklist enforcement | ❌ VULN-1 | Blacklisted users can bypass transfer check |
| 22 | Business Logic | Correct role gating on blacklist_add | ❌ VULN-3 | Uses master_authority instead of config.blacklister |
| 23 | Business Logic | Freeze authority initialized | ❌ VULN-5 | Freeze authority not transferred to config PDA in initialize |
| 24 | Business Logic | Paused flag symmetry | ⚠️ INFO-1 | burn() does not check paused |
| 25 | Error Handling | Descriptive custom error codes | ✅ | Custom `StablecoinError` enum present |
| 26 | Error Handling | `require!` macros used | ✅ | Consistently used throughout |
| 27 | Admin Security | Timelock / multisig on master_authority | ⚠️ INFO-3 | No timelock; recommend Squads multisig for mainnet |
| 28 | Admin Security | 2-step authority transfer | ⚠️ INFO-4 | pending_master_authority field exists but is unused |

---

## Assumptions Made

- The mint account is created externally (before `initialize` is called) with Token-2022. The program does not create the mint itself.
- `config.freezer`, `config.blacklister`, `config.pauser` are all initialized to `master_authority` and can be delegated post-deploy.
- The `preset` field (1 = compliant mode, 0 = basic mode) gates compliance-related features. This distinction is relied upon heavily but its validation logic is not audited here (assumed correct per business requirements).
- Transfer hook enforcement is assumed to be fully implemented in an external program; this program only validates that one is configured.

## Known Limitations / Follow-up for Auditor

1. **VULN-1 (Blacklist Bypass)** is the most critical finding — a blacklisted user can immediately call `transfer` and bypass all compliance enforcement. Fix before any other work.
2. **Token-2022 transfer hooks**: `initialize_extra_account_meta_list` is a stub. If transfer hooks are used, the hook program's security posture is entirely out of scope for this review.
3. **`seize()` CPI path**: Uses raw `invoke_signed` with `spl_token_2022`. Auditor should verify the constructed instruction matches the token program's expectations exactly, especially for Token-2022 mints with active extensions.
4. **Minter list as a Vec**: Stored on-chain as a dynamic vector capped at 10. Auditor should verify `INIT_SPACE` accounts for the max-size Vec correctly — a miscalculation could cause allocation failures or heap corruption.
5. **No upgrade authority governance**: The program is presumably upgradeable. Recommend transferring upgrade authority to a Squads multisig or timelock before mainnet. An upgradeable program with a single keypair as authority is a centralization risk equivalent to admin key compromise.
6. **`blacklist_remove` closes to user-provided `destination`**: The `destination` is a `SystemAccount` (not gated to master_authority or the original blacklisted account). Confirm this is intentional — it means rent from closed blacklist entries can be redirected arbitrarily.

---

## Verdict

**Not production-grade in current state.**

There are 3 critical vulnerabilities (VULN-1, VULN-2, VULN-3) that must be fixed before the program can be considered for deployment. VULN-1 in particular completely voids the compliance/blacklist feature — the program's core compliance guarantee is bypassable by any blacklisted user.

| Severity | Count |
|---|---|
| 🔴 Critical | 3 |
| 🟠 High | 2 |
| 🟡 Medium | 3 |
| 🔵 Info / Low | 6 |

Recommend: fix all Critical and High findings, re-review, then commission a full professional audit before mainnet.
