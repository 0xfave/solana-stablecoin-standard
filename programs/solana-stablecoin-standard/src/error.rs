use anchor_lang::prelude::*;

#[error_code]
pub enum StablecoinError {
    #[msg("Unauthorized - caller is not the master authority")]
    Unauthorized,
    #[msg("Unauthorized - caller is not the minter")]
    UnauthorizedMinter,
    #[msg("Unauthorized - caller is not the freezer")]
    UnauthorizedFreezer,
    #[msg("Unauthorized - caller is not the pauser")]
    UnauthorizedPauser,
    #[msg("Unauthorized - caller is not the blacklister")]
    UnauthorizedBlacklister,
    #[msg("Unauthorized - caller is not the seizer")]
    UnauthorizedSeizer,
    #[msg("Account is blacklisted")]
    Blacklisted,
    #[msg("Account is not blacklisted")]
    NotBlacklisted,
    #[msg("Account is already blacklisted")]
    AlreadyBlacklisted,
    #[msg("Minting is paused")]
    MintPaused,
    #[msg("Overflow in arithmetic")]
    Overflow,
    #[msg("Not Compliant Token (SSS-2 required)")]
    NotCompliantMode,
    #[msg("Transfers are paused")]
    TransfersPaused,
    #[msg("Sender is blacklisted")]
    SenderBlacklisted,
    #[msg("Receiver is blacklisted")]
    ReceiverBlacklisted,
    #[msg("Invalid account")]
    InvalidAccount,
    #[msg("Cannot blacklist zero address")]
    BlacklistZeroAddress,
    #[msg("Invalid blacklist account")]
    InvalidBlacklistAccount,
    #[msg("Transfer hook must be set in compliant mode")]
    TransferHookRequired,
    #[msg("Address is already a minter")]
    AlreadyMinter,
    #[msg("Minter not found")]
    MinterNotFound,
    #[msg("Too many minters (max 10)")]
    TooManyMinters,
    #[msg("Source and destination accounts cannot be the same")]
    SameAccount,
    #[msg("Burning is paused")]
    BurnPaused,
    #[msg("No pending master authority transfer for this account")]
    NoPendingTransfer,
}
