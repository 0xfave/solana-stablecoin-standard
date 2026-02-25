use anchor_lang::prelude::*;

#[error_code]
pub enum StablecoinError {
    #[msg("Unauthorized - caller is not the owner")]
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
    #[msg("Not Compliant Token")]
    NotCompliantMode,
}
