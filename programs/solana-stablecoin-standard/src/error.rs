use anchor_lang::prelude::*;


#[error_code]
pub enum SssError {
    #[msg("Signer is not the master authority")]
    Unauthorized,
    #[msg("Signer is not the authorized pauser")]
    UnauthorizedPauser,
    #[msg("Signer is not the authorized freezer")]
    UnauthorizedFreezer,
    #[msg("Signer is not the authorized minter")]
    UnauthorizedMinter,
    #[msg("Signer is not the authorized seizer")]
    UnauthorizedSeizer,
    #[msg("Signer is not the compliance module blacklister")]
    UnauthorizedBlacklister,
    #[msg("Signer is not the privacy module allowlist authority")]
    UnauthorizedAllowlistAuthority,
    #[msg("Module config field does not match the provided config account")]
    ModuleConfigMismatch,
    #[msg("Mint or burn is paused")]
    MintPaused,
    #[msg("Burn is paused")]
    BurnPaused,
    #[msg("Transfers are paused")]
    TransfersPaused,
    #[msg("Supply cap would be exceeded")]
    SupplyCapExceeded,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Sender is blacklisted")]
    SenderBlacklisted,
    #[msg("Receiver is blacklisted")]
    ReceiverBlacklisted,
    #[msg("Sender is not on the allowlist")]
    SenderNotAllowlisted,
    #[msg("Receiver is not on the allowlist")]
    ReceiverNotAllowlisted,
    #[msg("Source and destination accounts must be different")]
    SameAccount,
    #[msg("Address must not be the default (zero) pubkey")]
    InvalidAddress,
    #[msg("This pubkey is already a minter")]
    AlreadyMinter,
    #[msg("Minter not found in minters list")]
    MinterNotFound,
    #[msg("Maximum of 10 minters allowed")]
    TooManyMinters,
    #[msg("No pending authority transfer")]
    NoPendingTransfer,
    #[msg("Reason string exceeds 128 characters")]
    ReasonTooLong,
}
