// ============================================================
// Unit tests — SolanaStablecoin SDK (new modular architecture)
//
// HOW TO RUN:
//   npx jest sss_sdk_unit
//
// These tests use a mock connection and never hit the network.
// They verify discriminators, PDA derivation, account list
// shapes, and data serialisation — everything the SDK builds
// before handing off to a signer.
// ============================================================

import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  SolanaStablecoin,
  ComplianceClient,
  PrivacyClient,
  parseConfig,
  getInstructionDiscriminator,
  Signer,
  // REMOVED: Presets, PRESET — no preset concept in new architecture
} from "../src/index";
import { createHash } from "crypto";

// ─── Helpers ──────────────────────────────────────────────────────────────────

const PROGRAM_ID      = "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw";
const TOKEN_2022_ID   = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM  = "11111111111111111111111111111111";

function disc(name: string): Buffer {
  return createHash("sha256").update(`global:${name}`).digest().slice(0, 8);
}

/**
 * Capture the TransactionInstruction built inside an SDK method.
 *
 * WHY THIS APPROACH:
 * The SDK calls `tx.serialize()` on a transaction that was never really signed
 * (the mock signer doesn't hold a real keypair). `Transaction.serialize()` throws
 * "Transaction missing signature for fee payer" before `sendRawTransaction` is
 * ever called, so intercepting `sendRawTransaction` with raw bytes never works.
 *
 * Instead we spy on `Transaction.prototype.serialize`:
 *   1. Grab the last instruction off `this` (the Transaction instance) — that's
 *      our target, built and populated, right before serialization.
 *   2. Return an empty Buffer so `sendRawTransaction` receives something harmless
 *      and the mock returns "mockSignature" without error.
 */
async function captureIx(
  fn: () => Promise<string>
): Promise<TransactionInstruction> {
  let captured: TransactionInstruction | null = null;

  const spy = jest
    .spyOn(Transaction.prototype, "serialize")
    .mockImplementation(function (this: Transaction) {
      if (this.instructions.length > 0) {
        captured = this.instructions[this.instructions.length - 1];
      }
      return Buffer.alloc(0);
    });

  await fn().catch(() => {});
  spy.mockRestore();

  if (!captured) throw new Error("No instruction captured");
  return captured;
}

// ─── Fixtures ─────────────────────────────────────────────────────────────────

const AUTHORITY = new PublicKey("So11111111111111111111111111111111111111112");
const MINT_PK   = new PublicKey("7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU");
const USER_A    = new PublicKey("8opFLz5g2YwmmE28YJqJvw3f7bK8m2s4x8Y2w9QJzPQ");
const USER_B    = new PublicKey("9aKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgCbV");

const mockSigner: Signer = {
  publicKey: AUTHORITY,
  signTransaction: jest.fn().mockImplementation((tx: Transaction) => {
    tx.partialSign = jest.fn();
    return Promise.resolve(tx);
  }),
};

// Minimal mock connection — every method can be overridden per-test
function makeMockConnection(overrides: Record<string, jest.Mock> = {}): Connection {
  return {
    getMinimumBalanceForRentExemption: jest.fn().mockResolvedValue(1_000_000),
    getLatestBlockhash: jest.fn().mockResolvedValue({
      blockhash: "mockBlockhash",
      lastValidBlockHeight: 99999,
    }),
    sendRawTransaction: jest.fn().mockResolvedValue("mockSignature"),
    getAccountInfo: jest.fn().mockResolvedValue(null),
    getParsedAccountInfo: jest.fn().mockResolvedValue(null),
    getTokenAccountBalance: jest.fn().mockResolvedValue({ value: { amount: "0" } }),
    ...overrides,
  } as unknown as Connection;
}

// ─── Instruction discriminators ───────────────────────────────────────────────

describe("Instruction discriminators", () => {
  // Every discriminator must be 8 bytes and deterministic
  const ALL_INSTRUCTIONS = [
    // Core
    "initialize",
    "mint_tokens",       // CHANGED from "mint"
    "burn_tokens",       // CHANGED from "burn"
    "transfer",
    "freeze_account",
    "thaw_account",
    "add_minter",
    "remove_minter",
    "propose_master_authority",
    "accept_master_authority",
    "update_paused",
    "update_freezer",
    "update_pauser",
    "update_supply_cap",
    // Compliance module
    "attach_compliance_module",
    "detach_compliance_module",
    "blacklist_add",
    "blacklist_remove",
    "update_blacklister",
    "update_transfer_hook",
    // Privacy module
    "attach_privacy_module",
    "detach_privacy_module",
    "allowlist_add",
    "allowlist_remove",
    "update_allowlist_authority",
  ];

  ALL_INSTRUCTIONS.forEach((name) => {
    it(`discriminator for "${name}" is 8 bytes and deterministic`, () => {
      const d1 = disc(name);
      const d2 = getInstructionDiscriminator(name);
      expect(d1.length).toBe(8);
      expect(d2.length).toBe(8);
      expect(d1).toEqual(d2);
    });
  });

  // REMOVED: "mint" and "burn" — the program no longer has these names
  it('discriminator for "mint_tokens" differs from old "mint"', () => {
    expect(disc("mint_tokens")).not.toEqual(disc("mint"));
  });

  it('discriminator for "burn_tokens" differs from old "burn"', () => {
    expect(disc("burn_tokens")).not.toEqual(disc("burn"));
  });

  it("all discriminators are unique", () => {
    const hexSet = new Set(ALL_INSTRUCTIONS.map((n) => disc(n).toString("hex")));
    expect(hexSet.size).toBe(ALL_INSTRUCTIONS.length);
  });
});

// ─── PDA derivation ───────────────────────────────────────────────────────────

describe("PDA derivation", () => {
  const PROG = new PublicKey(PROGRAM_ID);

  it("derives config PDA from [stablecoin, mint]", async () => {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    expect(config).toBeInstanceOf(PublicKey);
    expect(PublicKey.isOnCurve(config.toBytes())).toBe(false); // PDAs are off-curve
  });

  it("derives compliance module PDA from [compliance, config]", async () => {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [compliancePda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), config.toBuffer()],
      PROG
    );
    expect(compliancePda).toBeInstanceOf(PublicKey);
    expect(PublicKey.isOnCurve(compliancePda.toBytes())).toBe(false);
  });

  it("derives privacy module PDA from [privacy, config]", async () => {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      PROG
    );
    expect(privacyPda).toBeInstanceOf(PublicKey);
    expect(PublicKey.isOnCurve(privacyPda.toBytes())).toBe(false);
  });

  it("derives blacklist entry PDA from [blacklist, config, wallet] using PROGRAM_ID", async () => {
    // CHANGED: program seed for blacklist is PROGRAM_ID, not mint
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [blacklistPda] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), USER_A.toBuffer()],
      PROG // CHANGED: was mint, now PROGRAM_ID
    );
    expect(blacklistPda).toBeInstanceOf(PublicKey);
    expect(PublicKey.isOnCurve(blacklistPda.toBytes())).toBe(false);
  });

  it("blacklist PDA differs per wallet — no collision between users", async () => {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [pdaA] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), USER_A.toBuffer()],
      PROG
    );
    const [pdaB] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), USER_B.toBuffer()],
      PROG
    );
    expect(pdaA.toBase58()).not.toBe(pdaB.toBase58());
  });

  it("derives allowlist entry PDA from [allowlist, privacy_module, wallet]", async () => {
    // CHANGED: allowlist seeds use privacy_module PDA as the middle seed, not config
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      PROG
    );
    const [allowlistPda] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), USER_A.toBuffer()],
      PROG
    );
    expect(allowlistPda).toBeInstanceOf(PublicKey);
    expect(PublicKey.isOnCurve(allowlistPda.toBytes())).toBe(false);
  });

  it("allowlist PDAs for two different users differ", async () => {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      PROG
    );
    const [pdaA] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), USER_A.toBuffer()],
      PROG
    );
    const [pdaB] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), USER_B.toBuffer()],
      PROG
    );
    expect(pdaA.toBase58()).not.toBe(pdaB.toBase58());
  });

  it("allowlist PDA keyed by privacy_module differs from one keyed by config", async () => {
    // Guard against accidentally regressing to old [allowlist, config, wallet] seeds
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      PROG
    );
    const [oldStyle] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), config.toBuffer(), USER_A.toBuffer()],
      PROG
    );
    const [newStyle] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), USER_A.toBuffer()],
      PROG
    );
    expect(oldStyle.toBase58()).not.toBe(newStyle.toBase58());
  });

  it("config PDAs for two different mints are different", async () => {
    const mint2 = new PublicKey("9zKZtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU");
    const [c1] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()],
      PROG
    );
    const [c2] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), mint2.toBuffer()],
      PROG
    );
    expect(c1.toBase58()).not.toBe(c2.toBase58());
  });
});

// ─── REMOVED: Presets / PRESET tests ─────────────────────────────────────────
//
// The preset concept no longer exists. SSS-1 vs SSS-2 is determined entirely
// by which module PDAs exist on-chain, not by a flag stored in StablecoinConfig.
// Tests for PRESET.SSS_1, PRESET.SSS_2 have been intentionally removed.
//
describe("Architecture — no preset flag", () => {
  it("SDK does not export PRESET", () => {
    // This import would fail at compile time; we verify at runtime via the module shape
    const sdkModule = require("../src/index");
    expect(sdkModule.PRESET).toBeUndefined();
    expect(sdkModule.Presets).toBeUndefined();
  });

  it("SolanaStablecoin instance has no preset getter", async () => {
    const conn = makeMockConnection();
    // Build a minimal config buffer for fetch() to parse
    const configBuf = buildConfigBuffer({});
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: configBuf });

    const s = await SolanaStablecoin.fetch(conn, MINT_PK);
    expect(s).not.toBeNull();
    expect((s as any).preset).toBeUndefined();
    expect((s as any)._preset).toBeUndefined();
  });

  it("SolanaStablecoin instance has no isCompliant getter", async () => {
    const conn = makeMockConnection();
    const configBuf = buildConfigBuffer({});
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: configBuf });

    const s = await SolanaStablecoin.fetch(conn, MINT_PK);
    expect((s as any).isCompliant).toBeUndefined();
  });
});

// ─── parseConfig ──────────────────────────────────────────────────────────────

/** Serialize a minimal StablecoinConfig buffer matching the new on-chain layout */
function buildConfigBuffer(opts: {
  masterAuthority?: PublicKey;
  mint?: PublicKey;
  paused?: boolean;
  supplyCap?: bigint;
  decimals?: number;
  bump?: number;
  pendingAuthority?: PublicKey;
  minters?: PublicKey[];
  freezer?: PublicKey;
  pauser?: PublicKey;
}): Buffer {
  const {
    masterAuthority = AUTHORITY,
    mint             = MINT_PK,
    paused           = false,
    supplyCap,
    decimals         = 6,
    bump             = 254,
    pendingAuthority,
    minters          = [AUTHORITY],
    freezer          = AUTHORITY,
    pauser           = AUTHORITY,
  } = opts;

  const parts: Buffer[] = [];

  // 8-byte discriminator (sha256 of "account:StablecoinConfig")
  const discriminator = createHash("sha256")
    .update("account:StablecoinConfig")
    .digest()
    .slice(0, 8);
  parts.push(discriminator);

  // master_authority (32)
  parts.push(masterAuthority.toBuffer());
  // mint (32)
  parts.push(mint.toBuffer());
  // paused (1)
  parts.push(Buffer.from([paused ? 1 : 0]));
  // supply_cap: Option<u64>
  if (supplyCap !== undefined) {
    const b = Buffer.alloc(9);
    b.writeUInt8(1, 0);
    b.writeBigUInt64LE(supplyCap, 1);
    parts.push(b);
  } else {
    parts.push(Buffer.from([0]));
  }
  // decimals (1)
  parts.push(Buffer.from([decimals]));
  // bump (1)
  parts.push(Buffer.from([bump]));
  // pending_master_authority: Option<Pubkey>
  if (pendingAuthority) {
    parts.push(Buffer.from([1]));
    parts.push(pendingAuthority.toBuffer());
  } else {
    parts.push(Buffer.from([0]));
  }
  // minters: Vec<Pubkey>
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32LE(minters.length, 0);
  parts.push(lenBuf);
  for (const m of minters) parts.push(m.toBuffer());
  // freezer (32)
  parts.push(freezer.toBuffer());
  // pauser (32)
  parts.push(pauser.toBuffer());

  return Buffer.concat(parts);
}

describe("parseConfig", () => {
  it("parses master_authority and mint correctly", () => {
    const buf = buildConfigBuffer({
      masterAuthority: AUTHORITY,
      mint: MINT_PK,
    });
    const cfg = parseConfig(buf);
    expect(cfg.masterAuthority.toBase58()).toBe(AUTHORITY.toBase58());
    expect(cfg.mint.toBase58()).toBe(MINT_PK.toBase58());
  });

  it("parses paused flag", () => {
    expect(parseConfig(buildConfigBuffer({ paused: false })).paused).toBe(false);
    expect(parseConfig(buildConfigBuffer({ paused: true })).paused).toBe(true);
  });

  it("parses supply cap when present", () => {
    const cfg = parseConfig(buildConfigBuffer({ supplyCap: 999_000_000_000n }));
    expect(cfg.supplyCap).toBe(999_000_000_000n);
  });

  it("supply cap is undefined when absent", () => {
    const cfg = parseConfig(buildConfigBuffer({}));
    expect(cfg.supplyCap).toBeUndefined();
  });

  it("parses decimals", () => {
    expect(parseConfig(buildConfigBuffer({ decimals: 9 })).decimals).toBe(9);
    expect(parseConfig(buildConfigBuffer({ decimals: 0 })).decimals).toBe(0);
  });

  it("parses bump", () => {
    expect(parseConfig(buildConfigBuffer({ bump: 253 })).bump).toBe(253);
  });

  it("parses pendingMasterAuthority when set", () => {
    const cfg = parseConfig(buildConfigBuffer({ pendingAuthority: USER_A }));
    expect(cfg.pendingMasterAuthority?.toBase58()).toBe(USER_A.toBase58());
  });

  it("pendingMasterAuthority is undefined when absent", () => {
    const cfg = parseConfig(buildConfigBuffer({}));
    expect(cfg.pendingMasterAuthority).toBeUndefined();
  });

  it("parses minters vec", () => {
    const cfg = parseConfig(buildConfigBuffer({ minters: [USER_A, USER_B] }));
    expect(cfg.minters.length).toBe(2);
    expect(cfg.minters[0].toBase58()).toBe(USER_A.toBase58());
    expect(cfg.minters[1].toBase58()).toBe(USER_B.toBase58());
  });

  it("parses empty minters vec", () => {
    const cfg = parseConfig(buildConfigBuffer({ minters: [] }));
    expect(cfg.minters).toEqual([]);
  });

  it("parses freezer and pauser", () => {
    const cfg = parseConfig(buildConfigBuffer({ freezer: USER_A, pauser: USER_B }));
    expect(cfg.freezer.toBase58()).toBe(USER_A.toBase58());
    expect(cfg.pauser.toBase58()).toBe(USER_B.toBase58());
  });

  // REMOVED: preset and blacklister fields — they no longer exist in StablecoinConfig
  it("does not have preset field", () => {
    const cfg = parseConfig(buildConfigBuffer({})) as any;
    expect(cfg.preset).toBeUndefined();
  });

  it("does not have blacklister field", () => {
    const cfg = parseConfig(buildConfigBuffer({})) as any;
    expect(cfg.blacklister).toBeUndefined();
  });

  it("does not have transfer_hook_program field", () => {
    const cfg = parseConfig(buildConfigBuffer({})) as any;
    expect(cfg.transferHookProgram).toBeUndefined();
    expect(cfg.transfer_hook_program).toBeUndefined();
  });

  it("returns null from SolanaStablecoin.fetch when discriminator is wrong", async () => {
    const conn = makeMockConnection();
    const badBuf = Buffer.alloc(64, 0); // all zeros — wrong discriminator
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: badBuf });
    const result = await SolanaStablecoin.fetch(conn, MINT_PK);
    expect(result).toBeNull();
  });

  it("returns null from SolanaStablecoin.fetch when account does not exist", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue(null);
    const result = await SolanaStablecoin.fetch(conn, MINT_PK);
    expect(result).toBeNull();
  });
});

// ─── mint_tokens instruction ──────────────────────────────────────────────────

describe("mint_tokens instruction", () => {
  it("uses discriminator for mint_tokens (not mint)", async () => {
    const conn = makeMockConnection();
    const configBuf = buildConfigBuffer({});
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: configBuf });

    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;
    const ix = await captureIx(() =>
      s.mint({ recipient: USER_A, amount: 1_000_000, minter: mockSigner })
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("mint_tokens"));
    expect(ix.data.slice(0, 8)).not.toEqual(disc("mint"));
  });

  it("encodes amount as little-endian u64", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.mint({ recipient: USER_A, amount: 500_000_000, minter: mockSigner })
    );

    const encoded = ix.data.readBigUInt64LE(8);
    expect(encoded).toBe(500_000_000n);
  });

  it("account list has 5 keys: config, mint, destination, minter, token_program", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.mint({ recipient: USER_A, amount: 1_000, minter: mockSigner })
    );

    expect(ix.keys.length).toBe(5);
    expect(ix.keys[3].isSigner).toBe(true); // minter
    expect(ix.keys[4].pubkey.toBase58()).toBe(TOKEN_2022_ID); // token_program
  });
});

// ─── burn_tokens instruction ──────────────────────────────────────────────────

describe("burn_tokens instruction", () => {
  it("uses discriminator for burn_tokens (not burn)", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.burn({ account: USER_A, amount: 100_000, authority: mockSigner })
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("burn_tokens"));
    expect(ix.data.slice(0, 8)).not.toEqual(disc("burn"));
  });

  it("encodes amount as little-endian u64", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.burn({ account: USER_A, amount: 123_456_789, authority: mockSigner })
    );

    expect(ix.data.readBigUInt64LE(8)).toBe(123_456_789n);
  });

  it("account list has 5 keys: config, mint, from, burner, token_program", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.burn({ account: USER_A, amount: 1_000, authority: mockSigner })
    );

    expect(ix.keys.length).toBe(5);
    expect(ix.keys[3].isSigner).toBe(true); // burner
    expect(ix.keys[4].pubkey.toBase58()).toBe(TOKEN_2022_ID);
  });
});

// ─── transfer instruction ──────────────────────────────────────────────────────

describe("transfer instruction", () => {
  it("uses discriminator for transfer", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.transfer({
        from: USER_A, to: USER_B,
        fromOwner: USER_A, toOwner: USER_B,
        amount: 1_000, authority: mockSigner,
      })
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("transfer"));
  });

  it("account list has 12 keys including all 6 module PDAs and mint", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.transfer({
        from: USER_A, to: USER_B,
        fromOwner: USER_A, toOwner: USER_B,
        amount: 500, authority: mockSigner,
      })
    );

    // config, compliance_module, sender_blacklist, receiver_blacklist,
    // privacy_module, sender_allowlist, receiver_allowlist, mint,
    // from, to, authority, token_program = 12
    expect(ix.keys.length).toBe(12);
  });

  it("module PDAs at index 1–6 are computed from fromOwner/toOwner not from/to", async () => {
    // Both from/to token accounts are USER_A but owners differ — the blacklist PDAs
    // must be derived from the owners, not the token accounts.
    const PROG = new PublicKey(PROGRAM_ID);
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;
    const [configPda] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()], PROG
    );
    const [expectedBlacklistA] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), configPda.toBuffer(), USER_A.toBuffer()], PROG
    );
    const [expectedBlacklistB] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), configPda.toBuffer(), USER_B.toBuffer()], PROG
    );

    const ix = await captureIx(() =>
      s.transfer({
        from: USER_A, to: USER_A, // same token account
        fromOwner: USER_A, toOwner: USER_B, // different owners
        amount: 1, authority: mockSigner,
      })
    );

    expect(ix.keys[2].pubkey.toBase58()).toBe(expectedBlacklistA.toBase58()); // sender_blacklist
    expect(ix.keys[3].pubkey.toBase58()).toBe(expectedBlacklistB.toBase58()); // receiver_blacklist
  });

  it("authority key at index 10 is the signer", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.transfer({
        from: USER_A, to: USER_B,
        fromOwner: USER_A, toOwner: USER_B,
        amount: 1, authority: mockSigner,
      })
    );

    expect(ix.keys[10].isSigner).toBe(true);
    expect(ix.keys[10].pubkey.toBase58()).toBe(AUTHORITY.toBase58());
  });

  it("last key is TOKEN_2022 program", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.transfer({
        from: USER_A, to: USER_B,
        fromOwner: USER_A, toOwner: USER_B,
        amount: 1, authority: mockSigner,
      })
    );

    expect(ix.keys[11].pubkey.toBase58()).toBe(TOKEN_2022_ID);
  });
});

// ─── attachComplianceModule instruction ───────────────────────────────────────

describe("attachComplianceModule instruction", () => {
  it("uses attach_compliance_module discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachComplianceModule(USER_A, mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("attach_compliance_module"));
  });

  it("account list has 5 keys — no mint, no TOKEN_2022", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachComplianceModule(USER_A, mockSigner)
    );

    // compliance_module, config, master_authority, authority (payer), system_program
    expect(ix.keys.length).toBe(5);
    expect(ix.keys[4].pubkey.toBase58()).toBe(SYSTEM_PROGRAM);
    // Must NOT contain TOKEN_2022
    const hasTok22 = ix.keys.some((k) => k.pubkey.toBase58() === TOKEN_2022_ID);
    expect(hasTok22).toBe(false);
  });

  it("encodes blacklister as first 32 bytes after discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachComplianceModule(USER_A, mockSigner)
    );

    const encodedBlacklister = new PublicKey(ix.data.slice(8, 40));
    expect(encodedBlacklister.toBase58()).toBe(USER_A.toBase58());
  });

  it("encodes optional transfer_hook as 0x00 when not provided", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachComplianceModule(USER_A, mockSigner) // no hook
    );

    expect(ix.data[40]).toBe(0); // Option::None for transfer_hook
  });

  it("encodes optional transfer_hook as 0x01 + pubkey when provided", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;
    const hookProg = USER_B;

    const ix = await captureIx(() =>
      s.attachComplianceModule(USER_A, mockSigner, hookProg)
    );

    expect(ix.data[40]).toBe(1); // Option::Some
    const encodedHook = new PublicKey(ix.data.slice(41, 73));
    expect(encodedHook.toBase58()).toBe(hookProg.toBase58());
  });
});

// ─── detachComplianceModule instruction ───────────────────────────────────────

describe("detachComplianceModule instruction", () => {
  it("uses detach_compliance_module discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.detachComplianceModule(mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("detach_compliance_module"));
  });

  it("account list has 4 keys — compliance_module, config, master_authority, authority", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.detachComplianceModule(mockSigner)
    );

    expect(ix.keys.length).toBe(4);
    // authority (index 3) is writable — receives rent from `close = authority`
    expect(ix.keys[3].isWritable).toBe(true);
  });
});

// ─── attachPrivacyModule instruction ──────────────────────────────────────────

describe("attachPrivacyModule instruction", () => {
  it("uses attach_privacy_module discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachPrivacyModule(USER_A, false, mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("attach_privacy_module"));
  });

  it("account list has 5 keys — no mint, no TOKEN_2022", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.attachPrivacyModule(USER_A, true, mockSigner)
    );

    expect(ix.keys.length).toBe(5);
    expect(ix.keys[4].pubkey.toBase58()).toBe(SYSTEM_PROGRAM);
    expect(ix.keys.some((k) => k.pubkey.toBase58() === TOKEN_2022_ID)).toBe(false);
  });

  it("encodes confidential_transfers_enabled correctly", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ixTrue = await captureIx(() =>
      s.attachPrivacyModule(USER_A, true, mockSigner)
    );
    const ixFalse = await captureIx(() =>
      s.attachPrivacyModule(USER_A, false, mockSigner)
    );

    expect(ixTrue.data[40]).toBe(1);
    expect(ixFalse.data[40]).toBe(0);
  });
});

// ─── blacklistAdd instruction ──────────────────────────────────────────────────

describe("blacklistAdd instruction", () => {
  it("uses blacklist_add discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.compliance.blacklistAdd(USER_A, "sanction", mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("blacklist_add"));
  });

  it("account list has 6 keys with compliance_module at index 1", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const PROG = new PublicKey(PROGRAM_ID);
    const [configPda] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()], PROG
    );
    const [compliancePda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), configPda.toBuffer()], PROG
    );

    const ix = await captureIx(() =>
      s.compliance.blacklistAdd(USER_A, "sanction", mockSigner)
    );

    // blacklist_entry, compliance_module, config, blacklister, wallet, system_program
    expect(ix.keys.length).toBe(6);
    expect(ix.keys[1].pubkey.toBase58()).toBe(compliancePda.toBase58());
  });

  it("encodes reason as length-prefixed string", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;
    const reason = "OFAC";

    const ix = await captureIx(() =>
      s.compliance.blacklistAdd(USER_A, reason, mockSigner)
    );

    const reasonLen = ix.data.readUInt32LE(8);
    expect(reasonLen).toBe(reason.length);
    expect(ix.data.slice(12, 12 + reason.length).toString()).toBe(reason);
  });
});

// ─── blacklistRemove instruction ───────────────────────────────────────────────

describe("blacklistRemove instruction", () => {
  it("uses blacklist_remove discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.compliance.blacklistRemove(USER_A, mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("blacklist_remove"));
  });

  it("account list has 6 keys; authority (index 5) is writable for rent reclaim", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.compliance.blacklistRemove(USER_A, mockSigner)
    );

    expect(ix.keys.length).toBe(6);
    expect(ix.keys[5].isWritable).toBe(true);
  });
});

// ─── seize instruction ─────────────────────────────────────────────────────────

describe("seize instruction", () => {
  it("uses seize discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.compliance.seize({
        from: USER_A, to: USER_B,
        sourceOwner: USER_A,
        amount: 1_000,
        seizer: mockSigner,
      })
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("seize"));
  });

  it("account list has 8 keys with compliance_module at index 1 and mint at index 2", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const PROG = new PublicKey(PROGRAM_ID);
    const [configPda] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()], PROG
    );
    const [compliancePda] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), configPda.toBuffer()], PROG
    );

    const ix = await captureIx(() =>
      s.compliance.seize({
        from: USER_A, to: USER_B,
        sourceOwner: USER_A,
        amount: 1_000,
        seizer: mockSigner,
      })
    );

    // config, compliance_module, mint, source_blacklist, from, to, seizer, TOKEN_2022
    expect(ix.keys.length).toBe(8);
    expect(ix.keys[1].pubkey.toBase58()).toBe(compliancePda.toBase58());
    expect(ix.keys[2].pubkey.toBase58()).toBe(MINT_PK.toBase58());
  });
});

// ─── allowlistAdd / allowlistRemove instructions ───────────────────────────────

describe("allowlistAdd instruction", () => {
  it("uses allowlist_add discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.privacy.allowlistAdd(USER_A, mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("allowlist_add"));
  });

  it("allowlist PDA at index 0 is keyed by [allowlist, privacy_module, wallet]", async () => {
    const PROG = new PublicKey(PROGRAM_ID);
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const [configPda] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), MINT_PK.toBuffer()], PROG
    );
    const [privacyPda] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), configPda.toBuffer()], PROG
    );
    const [expectedAllowlist] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyPda.toBuffer(), USER_A.toBuffer()], PROG
    );

    const ix = await captureIx(() =>
      s.privacy.allowlistAdd(USER_A, mockSigner)
    );

    expect(ix.keys[0].pubkey.toBase58()).toBe(expectedAllowlist.toBase58());
  });

  it("account list has 6 keys: allowlist_entry, privacy_module, config, authority, wallet, system_program", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.privacy.allowlistAdd(USER_A, mockSigner)
    );

    expect(ix.keys.length).toBe(6);
    expect(ix.keys[5].pubkey.toBase58()).toBe(SYSTEM_PROGRAM);
  });
});

describe("allowlistRemove instruction", () => {
  it("uses allowlist_remove discriminator", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.privacy.allowlistRemove(USER_A, mockSigner)
    );

    expect(ix.data.slice(0, 8)).toEqual(disc("allowlist_remove"));
  });

  it("authority at index 5 is writable (receives rent from close)", async () => {
    const conn = makeMockConnection();
    (conn as any).getAccountInfo = jest.fn().mockResolvedValue({ data: buildConfigBuffer({}) });
    const s = (await SolanaStablecoin.fetch(conn, MINT_PK))!;

    const ix = await captureIx(() =>
      s.privacy.allowlistRemove(USER_A, mockSigner)
    );

    expect(ix.keys.length).toBe(6);
    expect(ix.keys[5].isWritable).toBe(true);
  });
});

// ─── Signer interface ──────────────────────────────────────────────────────────

describe("Signer interface", () => {
  it("has publicKey property of type PublicKey", () => {
    expect(mockSigner.publicKey).toBeInstanceOf(PublicKey);
  });

  it("has signTransaction method", () => {
    expect(typeof mockSigner.signTransaction).toBe("function");
  });

  it("signTransaction returns a Transaction", async () => {
    const tx = new Transaction();
    const result = await mockSigner.signTransaction(tx);
    expect(result).toBeInstanceOf(Transaction);
  });
});

// ─── PublicKey validation ──────────────────────────────────────────────────────

describe("PublicKey validation", () => {
  it("accepts valid base58 public key", () => {
    const pk = new PublicKey("11111111111111111111111111111111");
    expect(pk.toString()).toBe("11111111111111111111111111111111");
  });

  it("throws on invalid public key", () => {
    expect(() => new PublicKey("not-a-pubkey")).toThrow();
  });

  it("program ID is a valid public key", () => {
    expect(() => new PublicKey(PROGRAM_ID)).not.toThrow();
  });

  it("TOKEN_2022_PROGRAM_ID is a valid public key", () => {
    expect(() => new PublicKey(TOKEN_2022_ID)).not.toThrow();
  });
});