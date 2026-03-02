import { Connection, PublicKey, Transaction } from "@solana/web3.js";
export declare const PRESET: {
    readonly SSS_1: 0;
    readonly SSS_2: 1;
};
export type Preset = (typeof PRESET)[keyof typeof PRESET];
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
export declare class SolanaStablecoin {
    private _connection;
    private _mint;
    private _config;
    private _authority;
    private _preset;
    private _programId;
    private constructor();
    static create(connection: Connection, params: CreateStablecoinParams): Promise<SolanaStablecoin>;
    static fetch(connection: Connection, mint: PublicKey): Promise<SolanaStablecoin | null>;
    get mintAddress(): PublicKey;
    get configAddress(): PublicKey;
    get authorityAddress(): PublicKey;
    get isCompliant(): boolean;
    getTotalSupply(): Promise<number>;
    mint(params: MintParams): Promise<string>;
    burn(params: BurnParams): Promise<string>;
    transfer(params: TransferParams): Promise<string>;
    get compliance(): ComplianceClient;
    addMinter(newMinter: PublicKey, authority: Signer): Promise<string>;
    removeMinter(minter: PublicKey, authority: Signer): Promise<string>;
}
export declare class ComplianceClient {
    private stablecoin;
    constructor(stablecoin: SolanaStablecoin);
    blacklistAdd(address: PublicKey, reason: string): Promise<string>;
    seize(params: SeizeParams): Promise<string>;
    freeze(account: PublicKey, authority: Signer): Promise<string>;
}
export declare const Presets: {
    readonly SSS_1: 0;
    readonly SSS_2: 1;
};
