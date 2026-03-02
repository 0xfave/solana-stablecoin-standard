import {
  Connection,
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
  Keypair,
} from "@solana/web3.js";
import { createHash } from "crypto";

export const PRESET = {
  SSS_1: 0,
  SSS_2: 1,
} as const;

export type Preset = (typeof PRESET)[keyof typeof PRESET];

function getInstructionDiscriminator(name: string): Buffer {
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

  private constructor(
    connection: Connection,
    mint: PublicKey,
    config: PublicKey,
    authority: PublicKey,
    preset: Preset
  ) {
    this._connection = connection;
    this._mint = mint;
    this._config = config;
    this._authority = authority;
    this._preset = preset;
    this._programId = new PublicKey(PROGRAM_ID);
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
    const lamports = await connection.getMinimumBalanceForRentExemption(82);

    tx.add(
      SystemProgram.createAccount({
        fromPubkey: authority.publicKey,
        newAccountPubkey: mintKeypair.publicKey,
        lamports,
        space: 82,
        programId: new PublicKey(TOKEN_2022_PROGRAM_ID),
      })
    );

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

    const data = configInfo.data;
    const discriminator = data.readUInt32LE(0);
    if (discriminator !== 1393578635) return null;

    const masterAuthority = new PublicKey(data.slice(4, 36));
    const mintAddr = new PublicKey(data.slice(36, 68));
    const preset = data[68];
    const paused = data[69] === 1;

    return new SolanaStablecoin(
      connection,
      mint,
      config,
      masterAuthority,
      preset as Preset
    );
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
    amountBuffer.writeBigUInt64BE(BigInt(amount));

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
      this.stablecoin.mintAddress
    );

    const reasonBytes = Buffer.alloc(200);
    reasonBytes.write(reason.slice(0, 200));

    const ix = new TransactionInstruction({
      programId: new PublicKey(PROGRAM_ID),
      keys: [
        { pubkey: config, isWritable: false, isSigner: false },
        { pubkey: blacklist, isWritable: true, isSigner: false },
        { pubkey: address, isWritable: false, isSigner: false },
        {
          pubkey: this.stablecoin.authorityAddress,
          isWritable: false,
          isSigner: true,
        },
        {
          pubkey: new PublicKey(SYSTEM_PROGRAM_ID),
          isWritable: false,
          isSigner: false,
        },
      ],
      data: Buffer.concat([Buffer.from([6]), reasonBytes]),
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
        { pubkey: seizer.publicKey, isWritable: false, isSigner: true },
        { pubkey: blacklist, isWritable: false, isSigner: false },
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

export const Presets = PRESET;
