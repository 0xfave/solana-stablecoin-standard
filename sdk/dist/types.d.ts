export interface MintRequest {
    id: string;
    user_wallet: string;
    amount: number;
    fiat_tx_id: string;
    custodian: string;
    requested_at: string;
    status: MintStatus;
    signature?: string;
    confirmed_at?: string;
    error?: string;
}
export declare enum MintStatus {
    Pending = "Pending",
    Processing = "Processing",
    AwaitingConfirmation = "AwaitingConfirmation",
    Confirmed = "Confirmed",
    Failed = "Failed",
    Cancelled = "Cancelled"
}
export interface BurnRequest {
    id: string;
    user_wallet: string;
    token_account: string;
    amount: number;
    fiat_destination: string;
    custodian: string;
    requested_at: string;
    status: BurnStatus;
    signature?: string;
    confirmed_at?: string;
    error?: string;
}
export declare enum BurnStatus {
    Pending = "Pending",
    Processing = "Processing",
    AwaitingConfirmation = "AwaitingConfirmation",
    Confirmed = "Confirmed",
    Failed = "Failed",
    Cancelled = "Cancelled"
}
export interface ApiResponse<T> {
    success: boolean;
    data?: T;
    error?: string;
}
export interface ComplianceResult {
    address: string;
    allowed: boolean;
    reason?: string;
    rules_triggered: string[];
    risk_score: number;
}
export interface BlacklistEntry {
    address: string;
    reason: string;
    blacklister: string;
    timestamp: string;
    status: BlacklistStatus;
}
export declare enum BlacklistStatus {
    Active = "Active",
    Removed = "Removed",
    PendingRemoval = "PendingRemoval"
}
export interface OnChainEvent {
    event_type: EventType;
    signature: string;
    slot: number;
    timestamp: string;
    data: Record<string, unknown>;
}
export declare enum EventType {
    ConfigInitialized = "ConfigInitialized",
    TokensMinted = "TokensMinted",
    TokensBurned = "TokensBurned",
    AccountFrozen = "AccountFrozen",
    AccountThawed = "AccountThawed",
    AddedToBlacklist = "AddedToBlacklist",
    RemovedFromBlacklist = "RemovedFromBlacklist",
    TokensSeized = "TokensSeized",
    TransferHookUpdated = "TransferHookUpdated",
    PausedChanged = "PausedChanged",
    MinterUpdated = "MinterUpdated",
    FreezerUpdated = "FreezerUpdated",
    PauserUpdated = "PauserUpdated",
    BlacklisterUpdated = "BlacklisterUpdated"
}
export interface IndexedEvent {
    id: string;
    event_type: EventType;
    signature: string;
    slot: number;
    timestamp: number;
    data: Record<string, unknown>;
    processed: boolean;
}
