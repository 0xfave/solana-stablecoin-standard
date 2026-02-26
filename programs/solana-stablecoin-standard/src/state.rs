use anchor_lang::prelude::*;

#[account]
#[derive(Debug, InitSpace)]
pub struct StablecoinConfig {
    pub master_authority: Pubkey,
    pub mint: Pubkey,
    pub preset: u8,
    pub paused: bool,
    pub supply_cap: Option<u64>,
    pub transfer_hook_program: Option<Pubkey>,
    pub decimals: u8,
    pub bump: u8,
    pub pending_master_authority: Option<Pubkey>,
}

#[account]
#[derive(Debug, InitSpace)]
pub struct BlacklistEntry {
    pub blacklister: Pubkey,
    #[max_len(200)]
    pub reason: String,
    pub timestamp: i64,
    pub bump: u8,
}
