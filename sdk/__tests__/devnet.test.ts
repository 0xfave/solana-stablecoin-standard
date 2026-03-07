// ============================================================
// Live devnet integration tests — SolanaStablecoin SDK
//
// HOW TO RUN:
//   1. Set env var with a funded devnet wallet:
//      export PRIVATE_KEY="5Kb8kLf9zgWQnogidDA76MzPL6TsZZY36hWXMssSzNydYXYB9KF"  # base58
//   2. npx jest sss_sdk_devnet --testTimeout=60000 --runInBand
//
// WHAT THESE TESTS DO:
//   Each describe block is an independent lifecycle. Tests within
//   a block run in order (--runInBand) and share state via closure.
//
//   Block 1 — SSS-1 core: create, mint, burn, transfer, pause
//   Block 2 — SSS-2 compliance: attach module, blacklist, seize, detach
//   Block 3 — Privacy module: attach, allowlist, transfer gate, detach
//   Block 4 — Module lifecycle: upgrade SSS-1 → SSS-2 → SSS-2+, then downgrade
//   Block 5 — Authority transfer: propose + accept two-step flow
//   Block 6 — Read existing on-chain state: fetch + parseConfig
// ============================================================

import {
  clusterApiUrl,
  Connection,
  Keypair,
  PublicKey,
  sendAndConfirmTransaction,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token";
import { createHash } from "crypto";

import {
  SolanaStablecoin,
  ComplianceClient,
  PrivacyClient,
  parseConfig,
  getInstructionDiscriminator,
  Signer as SdkSigner,
} from "../src/index";

// ─── Helpers ──────────────────────────────────────────────────────────────────

const DEVNET = clusterApiUrl("devnet");
const AIRDROP_AMOUNT = 2_000_000_000; // 2 SOL
const CONFIRM = { commitment: "confirmed" as const };

/** Wrap a Keypair as an SdkSigner */
function asSigner(kp: Keypair): SdkSigner {
  return {
    publicKey: kp.publicKey,
    signTransaction: async (tx: Transaction) => {
      tx.partialSign(kp);
      return tx;
    },
  };
}

/** Load keypair from PRIVATE_KEY env var (base58 string) or generate a fresh one */
function loadPayer(): Keypair {
  const raw = process.env.PRIVATE_KEY;
  if (raw) {
    const bs58module = require("bs58");
    const decode = bs58module.default?.decode ?? bs58module.decode;
    return Keypair.fromSecretKey(decode(raw));
  }
  console.warn(
    "PRIVATE_KEY not set — generating a throwaway keypair. " +
    "Airdrop will be requested automatically."
  );
  return Keypair.generate();
}

/** Request airdrop and wait for confirmation */
async function airdropIfNeeded(
  connection: Connection,
  pubkey: PublicKey,
  minLamports = 1_000_000_000
): Promise<void> {
  const balance = await connection.getBalance(pubkey, "confirmed");
  if (balance < minLamports) {
    console.log(`  Airdropping 2 SOL to ${pubkey.toBase58().slice(0, 8)}...`);
    const sig = await connection.requestAirdrop(pubkey, AIRDROP_AMOUNT);
    await connection.confirmTransaction(sig, "confirmed");
    console.log("  Airdrop confirmed.");
  }
}

/** Create an ATA and return its address */
async function createATA(
  connection: Connection,
  payer: Keypair,
  owner: PublicKey,
  mint: PublicKey
): Promise<PublicKey> {
  const ata = getAssociatedTokenAddressSync(mint, owner, false, TOKEN_2022_PROGRAM_ID);
  const info = await connection.getAccountInfo(ata, "confirmed");
  if (info) return ata; // already exists

  const ix = createAssociatedTokenAccountInstruction(
    payer.publicKey,
    ata,
    owner,
    mint,
    TOKEN_2022_PROGRAM_ID
  );
  const tx = new Transaction().add(ix);
  tx.feePayer = payer.publicKey;
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.sign(payer);
  await connection.sendRawTransaction(tx.serialize());
  await connection.confirmTransaction(
    await connection.sendRawTransaction(tx.serialize()),
    "confirmed"
  );
  return ata;
}

/** Poll until an account appears or timeout */
async function waitForAccount(
  connection: Connection,
  address: PublicKey,
  timeoutMs = 30_000
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const info = await connection.getAccountInfo(address, "confirmed");
    if (info) return;
    await new Promise((r) => setTimeout(r, 2000));
  }
  throw new Error(`Account ${address.toBase58()} did not appear within ${timeoutMs}ms`);
}

/** Poll until an account disappears (closed) */
async function waitForAccountClose(
  connection: Connection,
  address: PublicKey,
  timeoutMs = 30_000
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const info = await connection.getAccountInfo(address, "confirmed");
    if (!info) return;
    await new Promise((r) => setTimeout(r, 2000));
  }
  throw new Error(`Account ${address.toBase58()} was not closed within ${timeoutMs}ms`);
}

/** Confirm a transaction signature */
async function confirm(connection: Connection, sig: string): Promise<void> {
  const { blockhash, lastValidBlockHeight } =
    await connection.getLatestBlockhash();
  const result = await connection.confirmTransaction(
    { signature: sig, blockhash, lastValidBlockHeight },
    "confirmed"
  );
  if (result.value.err) {
    const tx = await connection.getTransaction(sig, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    const logs = tx?.meta?.logMessages?.join("\n") ?? "(no logs)";
    throw new Error(
      `Transaction ${sig.slice(0, 8)} failed on-chain:\n${JSON.stringify(result.value.err)}\n${logs}`
    );
  }
}

/** Read token balance */
async function tokenBalance(
  connection: Connection,
  ata: PublicKey
): Promise<bigint> {
  const info = await connection.getTokenAccountBalance(ata, "confirmed");
  return BigInt(info.value.amount);
}

// ─── Test suites ──────────────────────────────────────────────────────────────

describe("Live devnet — SSS-1 core (no modules)", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);
  const user = Keypair.generate();

  let stablecoin: SolanaStablecoin;
  let payerATA: PublicKey;
  let userATA: PublicKey;

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);
  }, 60_000);

  it("creates a stablecoin (SSS-1, no modules)", async () => {
    stablecoin = await SolanaStablecoin.create(connection, {
      name: "Test USD",
      symbol: "tUSD",
      decimals: 6,
      supplyCap: 1_000_000_000_000,
      authority: payerSigner,
    });

    await waitForAccount(connection, stablecoin.configAddress);

    const config = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    expect(config).not.toBeNull();
    console.log(`  Config PDA: ${stablecoin.configAddress.toBase58()}`);
    console.log(`  Mint:       ${stablecoin.mintAddress.toBase58()}`);
  }, 60_000);

  it("fetches and parses config via SolanaStablecoin.fetch", async () => {
    const fetched = await SolanaStablecoin.fetch(connection, stablecoin.mintAddress);
    expect(fetched).not.toBeNull();
    expect(fetched!.authorityAddress.toBase58()).toBe(payer.publicKey.toBase58());
    expect(fetched!.decimals).toBe(6);
    expect(fetched!.paused).toBe(false);
    console.log(`  Fetched and verified config.`);
  }, 30_000);

  it("creates ATAs for payer and user", async () => {
    await airdropIfNeeded(connection, payer.publicKey);
    payerATA = await createATA(connection, payer, payer.publicKey, stablecoin.mintAddress);
    userATA = await createATA(connection, payer, user.publicKey, stablecoin.mintAddress);
    console.log(`  Payer ATA: ${payerATA.toBase58()}`);
    console.log(`  User ATA:  ${userATA.toBase58()}`);
  }, 30_000);

  it("mints 1000 tokens to payer ATA", async () => {
    const sig = await stablecoin.mint({
      recipient: payerATA,
      amount: 1_000_000_000, // 1000 tokens at 6 decimals
      minter: payerSigner,
    });
    await confirm(connection, sig);

    const balance = await tokenBalance(connection, payerATA);
    expect(balance).toBe(1_000_000_000n);
    console.log(`  Minted. Balance: ${balance}`);
  }, 30_000);

  it("burns 200 tokens from payer ATA", async () => {
    const before = await tokenBalance(connection, payerATA);
    const sig = await stablecoin.burn({
      account: payerATA,
      amount: 200_000_000,
      authority: payerSigner,
    });
    await confirm(connection, sig);

    const after = await tokenBalance(connection, payerATA);
    expect(after).toBe(before - 200_000_000n);
    console.log(`  Burned. Balance: ${after}`);
  }, 30_000);

  it("transfers 100 tokens payer → user (SSS-1, no module checks)", async () => {
    const before = await tokenBalance(connection, userATA);
    const sig = await stablecoin.transfer({
      from: payerATA,
      to: userATA,
      fromOwner: payer.publicKey,
      toOwner: user.publicKey,
      amount: 100_000_000,
      authority: payerSigner,
    });
    await confirm(connection, sig);

    const after = await tokenBalance(connection, userATA);
    expect(after).toBe(before + 100_000_000n);
    console.log(`  Transferred. User balance: ${after}`);
  }, 30_000);

  it("pauses and unpauses the token", async () => {
    let sig = await stablecoin.updatePaused(true, payerSigner);
    await confirm(connection, sig);

    let info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const parsed = parseConfig(info!.data as Buffer);
    expect(parsed.paused).toBe(true);
    console.log("  Paused.");

    sig = await stablecoin.updatePaused(false, payerSigner);
    await confirm(connection, sig);

    info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const parsed2 = parseConfig(info!.data as Buffer);
    expect(parsed2.paused).toBe(false);
    console.log("  Unpaused.");
  }, 30_000);

  it("adds and removes a minter", async () => {
    const newMinter = Keypair.generate();

    let sig = await stablecoin.addMinter(newMinter.publicKey, payerSigner);
    await confirm(connection, sig);

    let info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    let config = parseConfig(info!.data as Buffer);
    expect(config.minters.map((k) => k.toBase58())).toContain(
      newMinter.publicKey.toBase58()
    );
    console.log(`  Added minter: ${newMinter.publicKey.toBase58().slice(0, 8)}`);

    sig = await stablecoin.removeMinter(newMinter.publicKey, payerSigner);
    await confirm(connection, sig);

    info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    config = parseConfig(info!.data as Buffer);
    expect(config.minters.map((k) => k.toBase58())).not.toContain(
      newMinter.publicKey.toBase58()
    );
    console.log("  Removed minter.");
  }, 30_000);

  it("updates freezer and pauser", async () => {
    const newFreezer = Keypair.generate().publicKey;
    const newPauser = Keypair.generate().publicKey;

    let sig = await stablecoin.updateFreezer(newFreezer, payerSigner);
    await confirm(connection, sig);

    sig = await stablecoin.updatePauser(newPauser, payerSigner);
    await confirm(connection, sig);

    const info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const config = parseConfig(info!.data as Buffer);
    expect(config.freezer.toBase58()).toBe(newFreezer.toBase58());
    expect(config.pauser.toBase58()).toBe(newPauser.toBase58());
    console.log("  Freezer and pauser updated.");
  }, 30_000);

  it("updates supply cap", async () => {
    const sig = await stablecoin.updateSupplyCap(500_000_000_000, payerSigner);
    await confirm(connection, sig);

    const info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const config = parseConfig(info!.data as Buffer);
    expect(config.supplyCap).toBe(500_000_000_000n);
    console.log("  Supply cap updated.");
  }, 30_000);
});

// ─────────────────────────────────────────────────────────────────────────────

describe("Live devnet — SSS-2 compliance module", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);
  const victim = Keypair.generate();
  const seizeDestination = Keypair.generate();

  let stablecoin: SolanaStablecoin;
  let payerATA: PublicKey;
  let victimATA: PublicKey;
  let seizeATA: PublicKey;
  let compliancePda: PublicKey;
  let blacklistPda: PublicKey;

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);
  }, 60_000);

  it("creates SSS-1 token and attaches compliance module (→ SSS-2)", async () => {
    stablecoin = await SolanaStablecoin.create(connection, {
      name: "Compliant USD",
      symbol: "cUSD",
      decimals: 6,
      authority: payerSigner,
    });
    await waitForAccount(connection, stablecoin.configAddress);

    const sig = await stablecoin.attachComplianceModule(
      payer.publicKey, // blacklister = authority
      payerSigner
    );
    await confirm(connection, sig);

    ;[compliancePda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), stablecoin.configAddress.toBuffer()],
      new PublicKey("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw")
    );
    await waitForAccount(connection, compliancePda);
    const isAttached = await stablecoin.compliance.isAttached();
    expect(isAttached).toBe(true);
    console.log(`  Compliance module attached: ${compliancePda.toBase58()}`);
  }, 60_000);

  it("creates ATAs and mints tokens to victim", async () => {
    payerATA  = await createATA(connection, payer, payer.publicKey,          stablecoin.mintAddress);
    victimATA = await createATA(connection, payer, victim.publicKey,         stablecoin.mintAddress);
    seizeATA  = await createATA(connection, payer, seizeDestination.publicKey, stablecoin.mintAddress);

    const sig = await stablecoin.mint({
      recipient: victimATA,
      amount: 1_000_000_000,
      minter: payerSigner,
    });
    await confirm(connection, sig);
    const balance = await tokenBalance(connection, victimATA);
    expect(balance).toBe(1_000_000_000n);
    console.log(`  Minted to victim. Balance: ${balance}`);
  }, 60_000);

  it("blacklists the victim wallet", async () => {
    const sig = await stablecoin.compliance.blacklistAdd(
      victim.publicKey,
      "Sanctions screening",
      payerSigner // blacklister
    );
    await confirm(connection, sig);

    ;[blacklistPda] = await PublicKey.findProgramAddress(
      [
        Buffer.from("blacklist"),
        stablecoin.configAddress.toBuffer(),
        victim.publicKey.toBuffer(),
      ],
      new PublicKey("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw")
    );
    await waitForAccount(connection, blacklistPda);
    const info = await connection.getAccountInfo(blacklistPda, "confirmed");
    expect(info).not.toBeNull();
    console.log(`  Blacklist entry created: ${blacklistPda.toBase58()}`);
  }, 30_000);

  it("seizes tokens from blacklisted victim", async () => {
    const beforeSeize = await tokenBalance(connection, seizeATA);
    const sig = await stablecoin.compliance.seize({
      from: victimATA,
      to: seizeATA,
      sourceOwner: victim.publicKey,
      amount: 500_000_000,
      seizer: payerSigner,
    });
    await confirm(connection, sig);

    const afterSeize = await tokenBalance(connection, seizeATA);
    expect(afterSeize).toBe(beforeSeize + 500_000_000n);
    console.log(`  Seized 500 tokens. Destination balance: ${afterSeize}`);
  }, 30_000);

  it("removes victim from blacklist", async () => {
    const sig = await stablecoin.compliance.blacklistRemove(
      victim.publicKey,
      payerSigner
    );
    await confirm(connection, sig);
    await waitForAccountClose(connection, blacklistPda);
    const info = await connection.getAccountInfo(blacklistPda, "confirmed");
    expect(info).toBeNull();
    console.log("  Blacklist entry closed.");
  }, 30_000);

  it("updates blacklister to a new address", async () => {
    const newBlacklister = Keypair.generate().publicKey;
    const sig = await stablecoin.compliance.updateBlacklister(
      newBlacklister,
      payerSigner
    );
    await confirm(connection, sig);
    console.log(`  Blacklister updated to ${newBlacklister.toBase58().slice(0, 8)}`);
  }, 30_000);

  it("detaches compliance module (→ SSS-1)", async () => {
    const sig = await stablecoin.detachComplianceModule(payerSigner);
    await confirm(connection, sig);
    await waitForAccountClose(connection, compliancePda);
    const isAttached = await stablecoin.compliance.isAttached();
    expect(isAttached).toBe(false);
    console.log("  Compliance module detached.");
  }, 30_000);
});

// ─────────────────────────────────────────────────────────────────────────────

describe("Live devnet — Privacy module", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);
  const userA = Keypair.generate();
  const userB = Keypair.generate();

  let stablecoin: SolanaStablecoin;
  let ataA: PublicKey;
  let ataB: PublicKey;
  let privacyPda: PublicKey;
  let allowlistA: PublicKey;
  let allowlistB: PublicKey;

  const PROG = new PublicKey("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw");

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);
  }, 60_000);

  it("creates token and attaches privacy module", async () => {
    stablecoin = await SolanaStablecoin.create(connection, {
      name: "Private USD",
      symbol: "pUSD",
      decimals: 6,
      authority: payerSigner,
    });
    await waitForAccount(connection, stablecoin.configAddress);

    const sig = await stablecoin.attachPrivacyModule(
      payer.publicKey, // allowlist_authority
      false,           // confidential_transfers_enabled
      payerSigner
    );
    await confirm(connection, sig);

    ;[privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), stablecoin.configAddress.toBuffer()],
      PROG
    );
    await waitForAccount(connection, privacyPda);
    const isAttached = await stablecoin.privacy.isAttached();
    expect(isAttached).toBe(true);
    console.log(`  Privacy module attached: ${privacyPda.toBase58()}`);
  }, 60_000);

  it("creates ATAs and mints tokens to userA", async () => {
    ataA = await createATA(connection, payer, userA.publicKey, stablecoin.mintAddress);
    ataB = await createATA(connection, payer, userB.publicKey, stablecoin.mintAddress);

    const sig = await stablecoin.mint({
      recipient: ataA,
      amount: 1_000_000_000,
      minter: payerSigner,
    });
    await confirm(connection, sig);
    console.log(`  Minted to userA.`);
  }, 60_000);

  it("adds userA and userB to allowlist", async () => {
    let sig = await stablecoin.privacy.allowlistAdd(userA.publicKey, payerSigner);
    await confirm(connection, sig);

    sig = await stablecoin.privacy.allowlistAdd(userB.publicKey, payerSigner);
    await confirm(connection, sig);

    ;[allowlistA] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), userA.publicKey.toBuffer()],
      PROG
    );
    ;[allowlistB] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), userB.publicKey.toBuffer()],
      PROG
    );

    await waitForAccount(connection, allowlistA);
    await waitForAccount(connection, allowlistB);

    const infoA = await connection.getAccountInfo(allowlistA, "confirmed");
    const infoB = await connection.getAccountInfo(allowlistB, "confirmed");
    expect(infoA).not.toBeNull();
    expect(infoB).not.toBeNull();
    console.log("  UserA and userB allowlisted.");
  }, 30_000);

  it("transfer succeeds when both parties are allowlisted", async () => {
    const before = await tokenBalance(connection, ataB);
    const sig = await stablecoin.transfer({
      from: ataA,
      to: ataB,
      fromOwner: payer.publicKey,  // payer owns ataA — payer can sign
      toOwner: userB.publicKey,
      amount: 100_000_000,
      authority: payerSigner,
    });
    await confirm(connection, sig);
  
    const after = await tokenBalance(connection, ataB);
    expect(after).toBe(before + 100_000_000n);
    console.log(`  Transfer succeeded. UserB balance: ${after}`);
  }, 30_000);

  it("removes userA from allowlist", async () => {
    const sig = await stablecoin.privacy.allowlistRemove(userA.publicKey, payerSigner);
    await confirm(connection, sig);
    await waitForAccountClose(connection, allowlistA);
    const info = await connection.getAccountInfo(allowlistA, "confirmed");
    expect(info).toBeNull();
    console.log("  UserA removed from allowlist.");
  }, 30_000);

  it("updates allowlist authority", async () => {
    const newAuthority = Keypair.generate().publicKey;
    const sig = await stablecoin.privacy.updateAllowlistAuthority(
      newAuthority,
      payerSigner
    );
    await confirm(connection, sig);
    console.log(`  Allowlist authority updated to ${newAuthority.toBase58().slice(0, 8)}`);
  }, 30_000);

  it("detaches privacy module", async () => {
    const sig = await stablecoin.detachPrivacyModule(payerSigner);
    await confirm(connection, sig);
    await waitForAccountClose(connection, privacyPda);
    const isAttached = await stablecoin.privacy.isAttached();
    expect(isAttached).toBe(false);
    console.log("  Privacy module detached.");
  }, 30_000);
});

// ─────────────────────────────────────────────────────────────────────────────

describe("Live devnet — Module lifecycle (upgrade + downgrade)", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);

  let stablecoin: SolanaStablecoin;

  const PROG = new PublicKey("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw");

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);
  }, 60_000);

  it("creates a bare SSS-1 token", async () => {
    stablecoin = await SolanaStablecoin.create(connection, {
      name: "Lifecycle USD",
      symbol: "lUSD",
      decimals: 6,
      authority: payerSigner,
    });
    await waitForAccount(connection, stablecoin.configAddress);

    const compIsAttached = await stablecoin.compliance.isAttached();
    const privIsAttached = await stablecoin.privacy.isAttached();
    expect(compIsAttached).toBe(false);
    expect(privIsAttached).toBe(false);
    console.log("  SSS-1: no modules attached.");
  }, 60_000);

  it("upgrades to SSS-2 by attaching compliance module", async () => {
    const sig = await stablecoin.attachComplianceModule(
      payer.publicKey,
      payerSigner
    );
    await confirm(connection, sig);

    const [compPda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), stablecoin.configAddress.toBuffer()],
      PROG
    );
    await waitForAccount(connection, compPda);
    expect(await stablecoin.compliance.isAttached()).toBe(true);
    expect(await stablecoin.privacy.isAttached()).toBe(false);
    console.log("  SSS-2: compliance module attached.");
  }, 30_000);

  it("upgrades to SSS-2+ by attaching privacy module on top", async () => {
    const sig = await stablecoin.attachPrivacyModule(
      payer.publicKey,
      false,
      payerSigner
    );
    await confirm(connection, sig);

    const [privPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), stablecoin.configAddress.toBuffer()],
      PROG
    );
    await waitForAccount(connection, privPda);
    expect(await stablecoin.compliance.isAttached()).toBe(true);
    expect(await stablecoin.privacy.isAttached()).toBe(true);
    console.log("  SSS-2+: both modules attached.");
  }, 30_000);

  it("downgrades to SSS-2 by detaching privacy module", async () => {
    const [privPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), stablecoin.configAddress.toBuffer()],
      PROG
    );
    const sig = await stablecoin.detachPrivacyModule(payerSigner);
    await confirm(connection, sig);
    await waitForAccountClose(connection, privPda);
    expect(await stablecoin.privacy.isAttached()).toBe(false);
    expect(await stablecoin.compliance.isAttached()).toBe(true);
    console.log("  Back to SSS-2: privacy module detached.");
  }, 30_000);

  it("downgrades to SSS-1 by detaching compliance module", async () => {
    const [compPda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), stablecoin.configAddress.toBuffer()],
      PROG
    );
    const sig = await stablecoin.detachComplianceModule(payerSigner);
    await confirm(connection, sig);
    await waitForAccountClose(connection, compPda);
    expect(await stablecoin.compliance.isAttached()).toBe(false);
    console.log("  Back to SSS-1: all modules detached.");
  }, 30_000);
});

// ─────────────────────────────────────────────────────────────────────────────

describe("Live devnet — Two-step authority transfer", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);
  const newOwner = Keypair.generate();

  let stablecoin: SolanaStablecoin;

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);
    await airdropIfNeeded(connection, newOwner.publicKey, 100_000_000);
  }, 60_000);

  it("creates a token and proposes authority transfer", async () => {
    stablecoin = await SolanaStablecoin.create(connection, {
      name: "Transfer USD",
      symbol: "xUSD",
      decimals: 6,
      authority: payerSigner,
    });
    await waitForAccount(connection, stablecoin.configAddress);

    const sig = await stablecoin.proposeMasterAuthority(
      newOwner.publicKey,
      payerSigner
    );
    await confirm(connection, sig);

    const info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const config = parseConfig(info!.data as Buffer);
    expect(config.pendingMasterAuthority?.toBase58()).toBe(newOwner.publicKey.toBase58());
    console.log(`  Proposed new authority: ${newOwner.publicKey.toBase58().slice(0, 8)}`);
  }, 60_000);

  it("new owner accepts the authority transfer", async () => {
    const newOwnerSigner = asSigner(newOwner);
    const sig = await stablecoin.acceptMasterAuthority(newOwnerSigner);
    await confirm(connection, sig);

    const info = await connection.getAccountInfo(stablecoin.configAddress, "confirmed");
    const config = parseConfig(info!.data as Buffer);
    expect(config.masterAuthority.toBase58()).toBe(newOwner.publicKey.toBase58());
    expect(config.pendingMasterAuthority).toBeUndefined();
    console.log("  Authority transfer accepted. New authority is on-chain.");
  }, 30_000);
});

// ─────────────────────────────────────────────────────────────────────────────

describe("Live devnet — Fetch and inspect existing on-chain state", () => {
  const connection = new Connection(DEVNET, "confirmed");
  const payer = loadPayer();
  const payerSigner = asSigner(payer);

  let mintAddress: PublicKey;

  beforeAll(async () => {
    await airdropIfNeeded(connection, payer.publicKey);

    // Create a token so we have something to fetch
    const s = await SolanaStablecoin.create(connection, {
      name: "Fetch USD",
      symbol: "fUSD",
      decimals: 6,
      supplyCap: 9_999_999_000_000,
      authority: payerSigner,
    });
    await waitForAccount(connection, s.configAddress);
    mintAddress = s.mintAddress;
  }, 90_000);

  it("fetches the stablecoin by mint address", async () => {
    const s = await SolanaStablecoin.fetch(connection, mintAddress);
    expect(s).not.toBeNull();
    expect(s!.mintAddress.toBase58()).toBe(mintAddress.toBase58());
    expect(s!.decimals).toBe(6);
    console.log(`  Fetched stablecoin for mint ${mintAddress.toBase58().slice(0, 8)}`);
  }, 30_000);

  it("parseConfig returns correct field values", async () => {
    const [configPda] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), mintAddress.toBuffer()],
      new PublicKey("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw")
    );
    const info = await connection.getAccountInfo(configPda, "confirmed");
    expect(info).not.toBeNull();

    const config = parseConfig(info!.data as Buffer);
    expect(config.masterAuthority.toBase58()).toBe(payer.publicKey.toBase58());
    expect(config.mint.toBase58()).toBe(mintAddress.toBase58());
    expect(config.paused).toBe(false);
    expect(config.decimals).toBe(6);
    expect(config.supplyCap).toBe(9_999_999_000_000n);
    expect(config.minters.length).toBeGreaterThan(0);
    expect(config.minters[0].toBase58()).toBe(payer.publicKey.toBase58());
    console.log(`  Config fields verified.`);
    console.log(`  Minters: ${config.minters.map((k) => k.toBase58().slice(0, 8)).join(", ")}`);
    console.log(`  Supply cap: ${config.supplyCap}`);
  }, 30_000);

  it("returns null for a random mint that has no config", async () => {
    const randomMint = Keypair.generate().publicKey;
    const s = await SolanaStablecoin.fetch(connection, randomMint);
    expect(s).toBeNull();
    console.log("  Correctly returned null for unknown mint.");
  }, 15_000);
});