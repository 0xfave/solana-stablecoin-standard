import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  Keypair,
} from "@solana/web3.js";
import {
  ExtensionType,
  getMintLen,
  createInitializePermanentDelegateInstruction,
  createInitializeMintInstruction,
} from "@solana/spl-token";
import { createHash } from "crypto";

export function getInstructionDiscriminator(name: string): Buffer {
  const hash = createHash("sha256").update(`global:${name}`).digest();
  return hash.slice(0, 8);
}

export interface Signer {
  publicKey: PublicKey;
  signTransaction(tx: Transaction): Promise<Transaction>;
}

export interface CreateStablecoinParams {
  name: string;
  symbol: string;
  decimals: number;
  supplyCap?: number;
  authority: Signer;
}

export interface MintParams {
  recipient: PublicKey;
  amount: number;
  minter: Signer;
}

export interface TransferParams {
  from: PublicKey;
  to: PublicKey;
  fromOwner: PublicKey;
  toOwner: PublicKey;
  amount: number;
  authority: Signer;
}

export interface BurnParams {
  account: PublicKey;
  amount: number;
  authority: Signer;
}

export interface SeizeParams {
  from: PublicKey;
  to: PublicKey;
  sourceOwner: PublicKey;
  amount: number;
  seizer: Signer;
}

export interface StablecoinConfig {
  masterAuthority: PublicKey;
  mint: PublicKey;
  paused: boolean;
  supplyCap?: bigint;
  decimals: number;
  bump: number;
  pendingMasterAuthority?: PublicKey;
  minters: PublicKey[];
  freezer: PublicKey;
  pauser: PublicKey;
}

const PROGRAM_ID = "C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

export class SolanaStablecoin {
  private _connection: Connection;
  private _mint: PublicKey;
  private _config: PublicKey;
  private _authority: PublicKey;
  private _programId: PublicKey;
  private _decimals: number = 9;
  private _minters: PublicKey[] = [];
  private _freezer: PublicKey | null = null;
  private _pauser: PublicKey | null = null;
  private _paused: boolean = false;

  private constructor(
    connection: Connection,
    mint: PublicKey,
    config: PublicKey,
    authority: PublicKey,
    decimals: number = 9,
    minters: PublicKey[] = [],
    freezer: PublicKey | null = null,
    pauser: PublicKey | null = null,
    paused: boolean = false
  ) {
    this._connection = connection;
    this._mint = mint;
    this._config = config;
    this._authority = authority;
    this._programId = new PublicKey(PROGRAM_ID);
    this._decimals = decimals;
    this._minters = minters;
    this._freezer = freezer;
    this._pauser = pauser;
    this._paused = paused;
  }

  get minters(): PublicKey[] {
    return this._minters;
  }
  get freezer(): PublicKey | null {
    return this._freezer;
  }
  get pauser(): PublicKey | null {
    return this._pauser;
  }
  get paused(): boolean {
    return this._paused;
  }

  static async create(
    connection: Connection,
    params: CreateStablecoinParams
  ): Promise<SolanaStablecoin> {
    const { authority, decimals, supplyCap } = params;

    const mintKeypair = Keypair.generate();
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), mintKeypair.publicKey.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const tx = new Transaction();

    const extensions = [ExtensionType.PermanentDelegate];
    const mintSpace = getMintLen(extensions);
    const lamports = await connection.getMinimumBalanceForRentExemption(
      mintSpace
    );

    tx.add(
      SystemProgram.createAccount({
        fromPubkey: authority.publicKey,
        newAccountPubkey: mintKeypair.publicKey,
        lamports,
        space: mintSpace,
        programId: new PublicKey(TOKEN_2022_PROGRAM_ID),
      })
    );

    // Always initialize permanent delegate extension
    const initPermanentDelegateIx =
      createInitializePermanentDelegateInstruction(
        mintKeypair.publicKey,
        config,
        new PublicKey(TOKEN_2022_PROGRAM_ID)
      );
    tx.add(initPermanentDelegateIx);

    const initMintIx = createInitializeMintInstruction(
      mintKeypair.publicKey,
      decimals,
      authority.publicKey,
      authority.publicKey,
      new PublicKey(TOKEN_2022_PROGRAM_ID)
    );
    tx.add(initMintIx);

    const initIx = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: config, isWritable: true, isSigner: false },
        { pubkey: mintKeypair.publicKey, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: true, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("initialize"),
        supplyCap
          ? (() => {
              const buf = Buffer.alloc(9);
              buf.writeUInt8(1, 0);
              buf.writeBigUInt64LE(BigInt(supplyCap), 1);
              return buf;
            })()
          : Buffer.from([0]),
        Buffer.from([decimals]),
      ]),
    });
    tx.add(initIx);

    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

    const signedTx = await authority.signTransaction(tx);
    signedTx.partialSign(mintKeypair);
    await connection.sendRawTransaction(signedTx.serialize());

    return new SolanaStablecoin(
      connection,
      mintKeypair.publicKey,
      config,
      authority.publicKey,
      decimals
    );
  }

  static async fetch(
    connection: Connection,
    mint: PublicKey
  ): Promise<SolanaStablecoin | null> {
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), mint.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const configInfo = await connection.getAccountInfo(config);
    if (!configInfo?.data) return null;

    const data = configInfo.data as Buffer;
    if (data.length < 8) return null;

    const expectedDiscriminator = createHash("sha256")
      .update("account:StablecoinConfig")
      .digest()
      .slice(0, 8);

    const actualDiscriminator = data.slice(0, 8);

    if (!actualDiscriminator.equals(expectedDiscriminator)) {
      return null;
    }

    try {
      const parsed = parseConfig(data);
      return new SolanaStablecoin(
        connection,
        mint,
        config,
        parsed.masterAuthority,
        parsed.decimals,
        parsed.minters,
        parsed.freezer,
        parsed.pauser,
        parsed.paused
      );
    } catch (e) {
      console.error("Failed to parse config:", e);
      return null;
    }
  }

  get mintAddress(): PublicKey {
    return this._mint;
  }

  get configAddress(): PublicKey {
    return this._config;
  }

  get authorityAddress(): PublicKey {
    return this._authority;
  }

  get decimals(): number {
    return this._decimals;
  }

  get connection(): Connection {
    return this._connection;
  }

  async getTotalSupply(): Promise<number> {
    const mintInfo = await this._connection.getParsedAccountInfo(this._mint);
    if (!mintInfo.value?.data) return 0;

    const data = mintInfo.value.data as {
      parsed: { info: { supply: { uiAmountString: string } } };
    };
    return parseFloat(data.parsed?.info?.supply?.uiAmountString ?? "0");
  }

  async mint(params: MintParams): Promise<string> {
    const { recipient, amount, minter } = params;

    const amountBuffer = Buffer.alloc(8);
    const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
    view.setBigUint64(0, BigInt(amount), true);

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: this._mint, isWritable: true, isSigner: false },
        { pubkey: recipient, isWritable: true, isSigner: false },
        { pubkey: minter.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("mint_tokens"),
        amountBuffer,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = minter.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await minter.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async burn(params: BurnParams): Promise<string> {
    const { account, amount, authority } = params;

    const amountBuffer = Buffer.alloc(8);
    const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
    view.setBigUint64(0, BigInt(amount), true);

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: this._mint, isWritable: true, isSigner: false },
        { pubkey: account, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("burn_tokens"),
        amountBuffer,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async transfer(params: TransferParams): Promise<string> {
    const { from, to, fromOwner, toOwner, amount, authority } = params;

    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this._config.toBuffer()],
      this._programId
    );
    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), this._config.toBuffer()],
      this._programId
    );
    const [senderBlacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), this._config.toBuffer(), fromOwner.toBuffer()],
      this._programId
    );
    const [receiverBlacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), this._config.toBuffer(), toOwner.toBuffer()],
      this._programId
    );
    const [senderAllowlist] = await PublicKey.findProgramAddress(
      [
        Buffer.from("allowlist"),
        privacyModule.toBuffer(),
        fromOwner.toBuffer(),
      ],
      this._programId
    );
    const [receiverAllowlist] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyModule.toBuffer(), toOwner.toBuffer()],
      this._programId
    );

    const amountBuffer = Buffer.alloc(8);
    const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
    view.setBigUint64(0, BigInt(amount), true);

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: complianceModule, isWritable: false, isSigner: false },
        { pubkey: senderBlacklist, isWritable: false, isSigner: false },
        { pubkey: receiverBlacklist, isWritable: false, isSigner: false },
        { pubkey: privacyModule, isWritable: false, isSigner: false },
        { pubkey: senderAllowlist, isWritable: false, isSigner: false },
        { pubkey: receiverAllowlist, isWritable: false, isSigner: false },
        { pubkey: this._mint, isWritable: false, isSigner: false },
        { pubkey: from, isWritable: true, isSigner: false },
        { pubkey: to, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("transfer"),
        amountBuffer,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async addMinter(newMinter: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("add_minter"),
        newMinter.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async removeMinter(minter: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("remove_minter"),
        minter.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async proposeMasterAuthority(
    newAuthority: PublicKey,
    authority: Signer
  ): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("propose_master_authority"),
        newAuthority.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async acceptMasterAuthority(newAuthority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: newAuthority.publicKey, isWritable: false, isSigner: true },
      ],
      data: getInstructionDiscriminator("accept_master_authority"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = newAuthority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await newAuthority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async updatePaused(paused: boolean, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_paused"),
        Buffer.from([paused ? 1 : 0]),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async updateFreezer(
    newFreezer: PublicKey,
    authority: Signer
  ): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_freezer"),
        newFreezer.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async updatePauser(newPauser: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_pauser"),
        newPauser.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async updateSupplyCap(
    newSupplyCap: number | null,
    authority: Signer
  ): Promise<string> {
    const supplyCapBuffer = Buffer.alloc(9);
    if (newSupplyCap !== null) {
      supplyCapBuffer.writeUInt8(1, 0);
      supplyCapBuffer.writeBigUInt64LE(BigInt(newSupplyCap), 1);
    } else {
      supplyCapBuffer.writeUInt8(0, 0);
    }

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_supply_cap"),
        supplyCapBuffer,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async thawAccount(account: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: this._mint, isWritable: false, isSigner: false },
        { pubkey: account, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: getInstructionDiscriminator("thaw_account"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async attachComplianceModule(
    blacklister: PublicKey,
    authority: Signer,
    transferHookProgram?: PublicKey,
    permanentDelegate?: PublicKey
  ): Promise<string> {
    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this._config.toBuffer()],
      this._programId
    );

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: complianceModule, isWritable: true, isSigner: false },
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (payer)
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("attach_compliance_module"),
        blacklister.toBuffer(),
        Buffer.from([transferHookProgram ? 1 : 0]),
        ...(transferHookProgram ? [transferHookProgram.toBuffer()] : []),
        Buffer.from([permanentDelegate ? 1 : 0]),
        ...(permanentDelegate ? [permanentDelegate.toBuffer()] : []),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async detachComplianceModule(authority: Signer): Promise<string> {
    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this._config.toBuffer()],
      this._programId
    );

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: complianceModule, isWritable: true, isSigner: false },
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
      ],
      data: getInstructionDiscriminator("detach_compliance_module"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async attachPrivacyModule(
    allowlistAuthority: PublicKey,
    confidentialTransfersEnabled: boolean,
    authority: Signer
  ): Promise<string> {
    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), this._config.toBuffer()],
      this._programId
    );

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: privacyModule, isWritable: true, isSigner: false },
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (payer)
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("attach_privacy_module"),
        allowlistAuthority.toBuffer(),
        Buffer.from([confidentialTransfersEnabled ? 1 : 0]),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  async detachPrivacyModule(authority: Signer): Promise<string> {
    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), this._config.toBuffer()],
      this._programId
    );

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: privacyModule, isWritable: true, isSigner: false },
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
      ],
      data: getInstructionDiscriminator("detach_privacy_module"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (
      await this._connection.getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return this._connection.sendRawTransaction(signed.serialize());
  }

  get compliance(): ComplianceClient {
    return new ComplianceClient(this);
  }

  get privacy(): PrivacyClient {
    return new PrivacyClient(this);
  }
}

export class ComplianceClient {
  constructor(private stablecoin: SolanaStablecoin) {}

  get connection(): Connection {
    return this.stablecoin.connection;
  }

  get configAddress(): PublicKey {
    return this.stablecoin.configAddress;
  }

  get mintAddress(): PublicKey {
    return this.stablecoin.mintAddress;
  }

  // Helper: returns true if the compliance module PDA exists on-chain
  async isAttached(): Promise<boolean> {
    const programId = new PublicKey(PROGRAM_ID);
    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this.configAddress.toBuffer()],
      programId
    );
    const info = await this.connection.getAccountInfo(complianceModule);
    return info !== null && info.data.length > 0;
  }

  async blacklistAdd(
    address: PublicKey,
    reason: string,
    blacklister: Signer
  ): Promise<string> {
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), config.toBuffer()],
      programId
    );
    const [blacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()],
      programId
    );

    const reasonBytes = Buffer.from(reason.slice(0, 128));
    const reasonLengthBuffer = Buffer.alloc(4);
    reasonLengthBuffer.writeUInt32LE(reasonBytes.length, 0);

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: blacklist, isWritable: true, isSigner: false },
        { pubkey: complianceModule, isWritable: false, isSigner: false },
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: blacklister.publicKey, isWritable: true, isSigner: true },
        { pubkey: address, isWritable: false, isSigner: false },
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("blacklist_add"),
        reasonLengthBuffer,
        reasonBytes,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = blacklister.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await blacklister.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async blacklistRemove(
    address: PublicKey,
    authority: Signer
  ): Promise<string> {
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), config.toBuffer()],
      programId
    );
    const [blacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()],
      programId
    );

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: blacklist, isWritable: true, isSigner: false },
        { pubkey: complianceModule, isWritable: false, isSigner: false },
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // master_authority
        { pubkey: address, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
      ],
      data: getInstructionDiscriminator("blacklist_remove"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async updateBlacklister(
    newBlacklister: PublicKey,
    authority: Signer
  ): Promise<string> {
    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this.stablecoin.configAddress.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: complianceModule, isWritable: true, isSigner: false },
        {
          pubkey: this.stablecoin.configAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_blacklister"),
        newBlacklister.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async updateTransferHook(
    newHookProgram: PublicKey | null,
    authority: Signer
  ): Promise<string> {
    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), this.stablecoin.configAddress.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const hookData = Buffer.alloc(33);
    if (newHookProgram) {
      hookData.writeUInt8(1, 0);
      hookData.set(newHookProgram.toBuffer(), 1);
    } else {
      hookData.writeUInt8(0, 0);
    }

    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: complianceModule, isWritable: true, isSigner: false },
        {
          pubkey: this.stablecoin.configAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_transfer_hook"),
        hookData,
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async seize(params: SeizeParams): Promise<string> {
    const { from, to, sourceOwner, amount, seizer } = params;
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [complianceModule] = await PublicKey.findProgramAddress(
      [Buffer.from("compliance"), config.toBuffer()],
      programId
    );
    const [blacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), sourceOwner.toBuffer()],
      programId
    );

    const amountBuffer = Buffer.alloc(8);
    const view = new DataView(amountBuffer.buffer, amountBuffer.byteOffset, 8);
    view.setBigUint64(0, BigInt(amount), true);

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: complianceModule, isWritable: false, isSigner: false },
        {
          pubkey: this.stablecoin.mintAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: blacklist, isWritable: false, isSigner: false },
        { pubkey: from, isWritable: true, isSigner: false },
        { pubkey: to, isWritable: true, isSigner: false },
        { pubkey: seizer.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([getInstructionDiscriminator("seize"), amountBuffer]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = seizer.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await seizer.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async freeze(account: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        {
          pubkey: this.stablecoin.configAddress,
          isWritable: false,
          isSigner: false,
        },
        {
          pubkey: this.stablecoin.mintAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: account, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: getInstructionDiscriminator("freeze_account"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }

  async thaw(account: PublicKey, authority: Signer): Promise<string> {
    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        {
          pubkey: this.stablecoin.configAddress,
          isWritable: false,
          isSigner: false,
        },
        {
          pubkey: this.stablecoin.mintAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: account, isWritable: true, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
        {
          pubkey: new PublicKey(TOKEN_2022_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: getInstructionDiscriminator("thaw_account"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await this.connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return this.connection.sendRawTransaction(signed.serialize());
  }
}

export class PrivacyClient {
  constructor(private stablecoin: SolanaStablecoin) {}

  // Helper: returns true if the privacy module PDA exists on-chain
  async isAttached(): Promise<boolean> {
    const programId = new PublicKey(PROGRAM_ID);
    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), this.stablecoin.configAddress.toBuffer()],
      programId
    );
    const info = await this.stablecoin.connection.getAccountInfo(privacyModule);
    return info !== null && info.data.length > 0;
  }

  async allowlistAdd(address: PublicKey, authority: Signer): Promise<string> {
    const connection = this.stablecoin.connection;
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      programId
    );
    const [allowlist] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyModule.toBuffer(), address.toBuffer()],
      programId
    );

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: allowlist, isWritable: true, isSigner: false },
        { pubkey: privacyModule, isWritable: false, isSigner: false },
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // allowlist_authority (payer)
        { pubkey: address, isWritable: false, isSigner: false },
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: getInstructionDiscriminator("allowlist_add"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return connection.sendRawTransaction(signed.serialize());
  }

  async allowlistRemove(
    address: PublicKey,
    authority: Signer
  ): Promise<string> {
    const connection = this.stablecoin.connection;
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      programId
    );
    const [allowlist] = await PublicKey.findProgramAddress(
      [Buffer.from("allowlist"), privacyModule.toBuffer(), address.toBuffer()],
      programId
    );

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: allowlist, isWritable: true, isSigner: false },
        { pubkey: privacyModule, isWritable: false, isSigner: false },
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true }, // allowlist_authority
        { pubkey: address, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: true, isSigner: true }, // authority (receives rent)
      ],
      data: getInstructionDiscriminator("allowlist_remove"),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return connection.sendRawTransaction(signed.serialize());
  }

  async updateAllowlistAuthority(
    newAuthority: PublicKey,
    authority: Signer
  ): Promise<string> {
    const connection = this.stablecoin.connection;
    const config = this.stablecoin.configAddress;
    const programId = new PublicKey(PROGRAM_ID);

    const [privacyModule] = await PublicKey.findProgramAddress(
      [Buffer.from("privacy"), config.toBuffer()],
      programId
    );

    const ix = new TransactionInstruction({
      programId,
      keys: [
        { pubkey: privacyModule, isWritable: true, isSigner: false },
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: authority.publicKey, isWritable: false, isSigner: true },
      ],
      data: Buffer.concat([
        getInstructionDiscriminator("update_allowlist_authority"),
        newAuthority.toBuffer(),
      ]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = authority.publicKey;
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;

    const signed = await authority.signTransaction(tx);
    return connection.sendRawTransaction(signed.serialize());
  }
}

export function parseConfig(data: Buffer): StablecoinConfig {
  let offset = 8; // skip discriminator

  const masterAuthority = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;

  const mint = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;

  const paused = data[offset] === 1;
  offset += 1;

  // supply_cap: Option<u64>
  const hasSupplyCap = data[offset] === 1;
  offset += 1;
  let supplyCap: bigint | undefined;
  if (hasSupplyCap) {
    supplyCap = data.readBigUInt64LE(offset);
    offset += 8;
  }

  // decimals: u8
  const decimals = data[offset];
  offset += 1;

  // bump: u8
  const bump = data[offset];
  offset += 1;

  // pending_master_authority: Option<Pubkey>
  const hasPending = data[offset] === 1;
  offset += 1;
  let pendingMasterAuthority: PublicKey | undefined;
  if (hasPending) {
    pendingMasterAuthority = new PublicKey(data.slice(offset, offset + 32));
    offset += 32;
  }

  // minters: Vec<Pubkey>
  const mintersLen = data.readUInt32LE(offset);
  offset += 4;
  const minters: PublicKey[] = [];
  for (let m = 0; m < mintersLen && m < 10; m++) {
    minters.push(new PublicKey(data.slice(offset, offset + 32)));
    offset += 32;
  }

  // freezer: Pubkey
  const freezer = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;

  // pauser: Pubkey
  const pauser = new PublicKey(data.slice(offset, offset + 32));

  return {
    masterAuthority,
    mint,
    paused,
    supplyCap,
    decimals,
    bump,
    pendingMasterAuthority,
    minters,
    freezer,
    pauser,
  };
}
