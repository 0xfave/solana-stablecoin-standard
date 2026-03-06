import { Connection, PublicKey, Transaction } from "@solana/web3.js";
export declare function getInstructionDiscriminator(name: string): Buffer;
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
export declare class SolanaStablecoin {
    private _connection;
    private _mint;
    private _config;
    private _authority;
    private _programId;
    private _decimals;
    private _minters;
    private _freezer;
    private _pauser;
    private _paused;
    private constructor();
    get minters(): PublicKey[];
    get freezer(): PublicKey | null;
    get pauser(): PublicKey | null;
    get paused(): boolean;
    static create(connection: Connection, params: CreateStablecoinParams): Promise<SolanaStablecoin>;
    static fetch(connection: Connection, mint: PublicKey): Promise<SolanaStablecoin | null>;
    get mintAddress(): PublicKey;
    get configAddress(): PublicKey;
    get authorityAddress(): PublicKey;
    get decimals(): number;
    get connection(): Connection;
    getTotalSupply(): Promise<number>;
    mint(params: MintParams): Promise<string>;
    burn(params: BurnParams): Promise<string>;
    transfer(params: TransferParams): Promise<string>;
    addMinter(newMinter: PublicKey, authority: Signer): Promise<string>;
    removeMinter(minter: PublicKey, authority: Signer): Promise<string>;
    proposeMasterAuthority(newAuthority: PublicKey, authority: Signer): Promise<string>;
    acceptMasterAuthority(newAuthority: Signer): Promise<string>;
    updatePaused(paused: boolean, authority: Signer): Promise<string>;
    updateFreezer(newFreezer: PublicKey, authority: Signer): Promise<string>;
    updatePauser(newPauser: PublicKey, authority: Signer): Promise<string>;
    updateSupplyCap(newSupplyCap: number | null, authority: Signer): Promise<string>;
    thawAccount(account: PublicKey, authority: Signer): Promise<string>;
    attachComplianceModule(blacklister: PublicKey, authority: Signer, transferHookProgram?: PublicKey, permanentDelegate?: PublicKey): Promise<string>;
    detachComplianceModule(authority: Signer): Promise<string>;
    attachPrivacyModule(allowlistAuthority: PublicKey, confidentialTransfersEnabled: boolean, authority: Signer): Promise<string>;
    detachPrivacyModule(authority: Signer): Promise<string>;
    get compliance(): ComplianceClient;
    get privacy(): PrivacyClient;
}
export declare class ComplianceClient {
    private stablecoin;
    constructor(stablecoin: SolanaStablecoin);
    get connection(): Connection;
    get configAddress(): PublicKey;
    get mintAddress(): PublicKey;
    isAttached(): Promise<boolean>;
    blacklistAdd(address: PublicKey, reason: string, blacklister: Signer): Promise<string>;
    blacklistRemove(address: PublicKey, authority: Signer): Promise<string>;
    updateBlacklister(newBlacklister: PublicKey, authority: Signer): Promise<string>;
    updateTransferHook(newHookProgram: PublicKey | null, authority: Signer): Promise<string>;
    seize(params: SeizeParams): Promise<string>;
    freeze(account: PublicKey, authority: Signer): Promise<string>;
    thaw(account: PublicKey, authority: Signer): Promise<string>;
}
export declare class PrivacyClient {
    private stablecoin;
    constructor(stablecoin: SolanaStablecoin);
    isAttached(): Promise<boolean>;
    allowlistAdd(address: PublicKey, authority: Signer): Promise<string>;
    allowlistRemove(address: PublicKey, authority: Signer): Promise<string>;
    updateAllowlistAuthority(newAuthority: PublicKey, authority: Signer): Promise<string>;
}
export declare function parseConfig(data: Buffer): StablecoinConfig;
