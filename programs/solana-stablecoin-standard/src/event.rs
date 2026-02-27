use anchor_lang::prelude::*;

#[event]
pub struct ConfigInitialized {
    pub config: Pubkey,
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub preset: u8,
}

#[event]
pub struct PausedChanged {
    pub paused: bool,
}

#[event]
pub struct TransferHookUpdated {
    pub config: Pubkey,
    pub old_hook_program: Option<Pubkey>,
    pub new_hook_program: Option<Pubkey>,
}

#[event]
pub struct MinterUpdated {
    pub config: Pubkey,
    pub old_minter: Pubkey,
    pub new_minter: Pubkey,
}

#[event]
pub struct FreezerUpdated {
    pub config: Pubkey,
    pub old_freezer: Pubkey,
    pub new_freezer: Pubkey,
}

#[event]
pub struct PauserUpdated {
    pub config: Pubkey,
    pub old_pauser: Pubkey,
    pub new_pauser: Pubkey,
}

#[event]
pub struct BlacklisterUpdated {
    pub config: Pubkey,
    pub old_blacklister: Pubkey,
    pub new_blacklister: Pubkey,
}

#[event]
pub struct TokensMinted {
    pub mint: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub minter: Pubkey,
}

#[event]
pub struct TokensBurned {
    pub mint: Pubkey,
    pub from: Pubkey,
    pub amount: u64,
    pub burner: Pubkey,
}

#[event]
pub struct AccountFrozen {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub freezer: Pubkey,
}

#[event]
pub struct AccountThawed {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub freezer: Pubkey,
}

#[event]
pub struct AddedToBlacklist {
    pub config: Pubkey,
    pub target: Pubkey,
    pub reason: String,
    pub blacklister: Pubkey,
}

#[event]
pub struct RemovedFromBlacklist {
    pub config: Pubkey,
    pub target: Pubkey,
    pub blacklister: Pubkey,
}
