use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct StablecoinConfig {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub minter: Pubkey,                        // can mint
    pub freezer: Pubkey,                       // can freeze accounts
    pub pauser: Pubkey,                        // can pause minting
    pub blacklister: Pubkey,                   // SSS-2
    pub transfer_hook_program: Option<Pubkey>, // SSS-2
    pub permanent_delegate: Option<Pubkey>,    // SSS-2
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Blacklist {
    #[max_len(100)]
    pub entries: Vec<Pubkey>,
    pub bump: u8,
}
