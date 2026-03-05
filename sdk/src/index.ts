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

export const PRESET = {
  SSS_1: 0,
  SSS_2: 1,
} as const;

export type Preset = (typeof PRESET)[keyof typeof PRESET];

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
  preset: Preset;
  supplyCap?: number;
  authority: Signer;
  extensions?: {
    permanentDelegate?: boolean;
    transferHook?: boolean;
  };
}

export interface MintParams {
  recipient: PublicKey;
  amount: number;
  minter: Signer;
}

export interface TransferParams {
  from: PublicKey;
  to: PublicKey;
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
  amount: number;
  seizer: Signer;
}

export interface StablecoinConfig {
  masterAuthority: PublicKey;
  mint: PublicKey;
  preset: Preset;
  paused: boolean;
  supplyCap?: bigint;
  decimals: number;
  bump: number;
  pendingMasterAuthority?: PublicKey;
  minters: PublicKey[];
  freezer: PublicKey;
  pauser: PublicKey;
  blacklister: PublicKey;
}

const PROGRAM_ID = "Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6";
const TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

export class SolanaStablecoin {
  private _connection: Connection;
  private _mint: PublicKey;
  private _config: PublicKey;
  private _authority: PublicKey;
  private _preset: Preset;
  private _programId: PublicKey;
  private _decimals: number = 9;
  private _minters: PublicKey[] = [];
  private _freezer: PublicKey | null = null;
  private _pauser: PublicKey | null = null;
  private _blacklister: PublicKey | null = null;
  private _paused: boolean = false;

  private constructor(
    connection: Connection,
    mint: PublicKey,
    config: PublicKey,
    authority: PublicKey,
    preset: Preset,
    decimals: number = 9,
    minters: PublicKey[] = [],
    freezer: PublicKey | null = null,
    pauser: PublicKey | null = null,
    blacklister: PublicKey | null = null,
    paused: boolean = false
  ) {
    this._connection = connection;
    this._mint = mint;
    this._config = config;
    this._authority = authority;
    this._preset = preset;
    this._programId = new PublicKey(PROGRAM_ID);
    this._decimals = decimals;
    this._minters = minters;
    this._freezer = freezer;
    this._pauser = pauser;
    this._blacklister = blacklister;
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
  get blacklister(): PublicKey | null {
    return this._blacklister;
  }
  get paused(): boolean {
    return this._paused;
  }

  get preset(): Preset {
    return this._preset;
  }

  static async create(
    connection: Connection,
    params: CreateStablecoinParams
  ): Promise<SolanaStablecoin> {
    const { preset, authority, decimals, supplyCap } = params;

    const mintKeypair = Keypair.generate();
    const [config] = await PublicKey.findProgramAddress(
      [Buffer.from("stablecoin"), mintKeypair.publicKey.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const tx = new Transaction();

    // Calculate mint space - larger if preset=1 (SSS-2) needs Permanent Delegate extension
    const extensions =
      preset === PRESET.SSS_2 ? [ExtensionType.PermanentDelegate] : [];
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

    // For preset=1 (SSS-2), add Permanent Delegate extension
    if (preset === PRESET.SSS_2) {
      const initPermanentDelegateIx =
        createInitializePermanentDelegateInstruction(
          mintKeypair.publicKey,
          config,
          new PublicKey(TOKEN_2022_PROGRAM_ID)
        );
      tx.add(initPermanentDelegateIx);
    }

    const initMintIx = createInitializeMintInstruction(
      mintKeypair.publicKey,
      decimals,
      authority.publicKey, // temporary mint authority (will be changed by program)
      authority.publicKey, // temporary freeze authority
      new PublicKey(TOKEN_2022_PROGRAM_ID)
    );
    tx.add(initMintIx);

    const initIx = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: config, isWritable: true, isSigner: false },
        { pubkey: mintKeypair.publicKey, isWritable: true, isSigner: true },
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
        Buffer.from([preset]),
        supplyCap
          ? (() => {
              const buf = Buffer.alloc(9);
              buf.writeUInt8(1, 0);
              buf.writeBigUInt64BE(BigInt(supplyCap), 1);
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
      preset
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

    // ✅ Compute correct 8-byte Anchor discriminator
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
        parsed.preset,
        parsed.decimals,
        parsed.minters,
        parsed.freezer,
        parsed.pauser,
        parsed.blacklister,
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

  get isCompliant(): boolean {
    return this._preset === PRESET.SSS_2;
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
      data: Buffer.concat([getInstructionDiscriminator("mint"), amountBuffer]),
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
    amountBuffer.writeBigUInt64BE(BigInt(amount));

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
      data: Buffer.concat([getInstructionDiscriminator("burn"), amountBuffer]),
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
    const { from, to, amount, authority } = params;

    const [senderBlacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), this._config.toBuffer(), from.toBuffer()],
      this._mint
    );
    const [receiverBlacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), this._config.toBuffer(), to.toBuffer()],
      this._mint
    );

    const amountBuffer = Buffer.alloc(8);
    amountBuffer.writeBigUInt64BE(BigInt(amount));

    const ix = new TransactionInstruction({
      programId: this._programId,
      keys: [
        { pubkey: this._config, isWritable: false, isSigner: false },
        { pubkey: senderBlacklist, isWritable: false, isSigner: false },
        { pubkey: receiverBlacklist, isWritable: false, isSigner: false },
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

  get compliance(): ComplianceClient {
    return new ComplianceClient(this);
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
}

export class ComplianceClient {
  constructor(private stablecoin: SolanaStablecoin) {}

  async blacklistAdd(address: PublicKey, reason: string): Promise<string> {
    const config = this.stablecoin.configAddress;
    const [blacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), address.toBuffer()],
      new PublicKey(PROGRAM_ID)
    );

    const reasonBytes = Buffer.from(reason.slice(0, 200));
    const reasonLengthBuffer = Buffer.alloc(4);
    reasonLengthBuffer.writeUInt32LE(reasonBytes.length, 0);

    const discriminator = Buffer.from([
      0xfe, 0xb8, 0x83, 0xc8, 0x91, 0x32, 0x2b, 0xf4,
    ]);

    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: blacklist, isWritable: true, isSigner: false },
        { pubkey: config, isWritable: true, isSigner: false },
        {
          pubkey: this.stablecoin.authorityAddress,
          isWritable: true,
          isSigner: true,
        },
        { pubkey: address, isWritable: false, isSigner: false },
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([discriminator, reasonLengthBuffer, reasonBytes]),
    });

    const tx = new Transaction().add(ix);
    tx.feePayer = this.stablecoin.authorityAddress;
    tx.recentBlockhash = (
      await new Connection("").getLatestBlockhash()
    ).blockhash;

    return "";
  }

  async seize(params: SeizeParams): Promise<string> {
    const { from, to, amount, seizer } = params;
    const config = this.stablecoin.configAddress;
    const [blacklist] = await PublicKey.findProgramAddress(
      [Buffer.from("blacklist"), config.toBuffer(), from.toBuffer()],
      this.stablecoin.mintAddress
    );

    const amountBuffer = Buffer.alloc(8);
    amountBuffer.writeBigUInt64BE(BigInt(amount));

    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: config, isWritable: false, isSigner: false },
        {
          pubkey: this.stablecoin.mintAddress,
          isWritable: false,
          isSigner: false,
        },
        { pubkey: from, isWritable: true, isSigner: false },
        { pubkey: to, isWritable: true, isSigner: false },
        { pubkey: blacklist, isWritable: false, isSigner: false },
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
    tx.recentBlockhash = (
      await new Connection("").getLatestBlockhash()
    ).blockhash;

    const signed = await seizer.signTransaction(tx);
    return new Connection("").sendRawTransaction(signed.serialize());
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
    tx.recentBlockhash = (
      await new Connection("").getLatestBlockhash()
    ).blockhash;

    const signed = await authority.signTransaction(tx);
    return new Connection("").sendRawTransaction(signed.serialize());
  }
}

export function parseConfig(data: Buffer): StablecoinConfig {
  // 0-7:   discriminator
  // 8-39:  master_authority
  // 40-71: mint
  // 72:    preset
  // 73:    paused

  const masterAuthority = new PublicKey(data.slice(8, 40));
  const mint = new PublicKey(data.slice(40, 72));
  const preset = data[72] as Preset;
  const paused = data[73] === 1;

  let offset = 74;

  // supply_cap: Option<u64>
  const hasSupplyCap = data[offset] === 1;
  offset += 1;
  let supplyCap: bigint | undefined;
  if (hasSupplyCap) {
    supplyCap = data.readBigUInt64LE(offset);
    offset += 8;
  }

  // transfer_hook_program: Option<Pubkey>
  const hasTransferHook = data[offset] === 1;
  offset += 1;
  if (hasTransferHook) offset += 32;

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
  offset += 32;

  // blacklister: Pubkey
  const blacklister = new PublicKey(data.slice(offset, offset + 32));

  return {
    masterAuthority,
    mint,
    preset,
    paused,
    supplyCap,
    decimals,
    bump,
    pendingMasterAuthority,
    minters,
    freezer,
    pauser,
    blacklister,
  };
}

export const Presets = PRESET;
