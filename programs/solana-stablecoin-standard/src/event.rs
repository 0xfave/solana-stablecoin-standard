use anchor_lang::prelude::*;

#[event] 
pub struct ConfigInitialized { 
    pub config: Pubkey,
    pub authority: Pubkey,
    pub mint: Pubkey
}

#[event] 
pub struct ComplianceModuleAttached {
    pub config: Pubkey,
    pub blacklister: Pubkey,
    pub transfer_hook_program: Option<Pubkey>
}

#[event] 
pub struct ComplianceModuleDetached {
    pub config: Pubkey
}

#[event] 
pub struct PrivacyModuleAttached {
    pub config: Pubkey,
    pub allowlist_authority: Pubkey,
    pub confidential_transfers_enabled: bool
}

#[event]
pub struct PrivacyModuleDetached {
    pub config: Pubkey
}

#[event]
pub struct TokensMinted {
    pub mint: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub minter: Pubkey
}

#[event] 
pub struct TokensBurned {
    pub mint: Pubkey,
    pub from: Pubkey,
    pub amount: u64,
    pub burner: Pubkey
}

#[event] 
pub struct TokensSeized {
    pub mint: Pubkey,
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub seizer: Pubkey
}

#[event] 
pub struct TokensTransferred {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub authority: Pubkey
}

#[event] 
pub struct AccountFrozen {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub freezer: Pubkey
}

#[event] 
pub struct AccountThawed {
    pub account: Pubkey,
    pub mint: Pubkey,
    pub freezer: Pubkey
}

#[event] 
pub struct AddedToBlacklist {
    pub config: Pubkey,
    pub target: Pubkey,
    pub reason: String,
    pub blacklister: Pubkey
}

#[event] 
pub struct RemovedFromBlacklist {
    pub config: Pubkey,
    pub target: Pubkey,
    pub blacklister: Pubkey
}

#[event] 
pub struct AddedToAllowlist {
    pub config: Pubkey,
    pub wallet: Pubkey,
    pub approved_by: Pubkey
}

#[event] 
pub struct RemovedFromAllowlist {
    pub config: Pubkey,
    pub wallet: Pubkey
}

#[event] 
pub struct BlacklisterUpdated {
    pub config: Pubkey,
    pub old_blacklister: Pubkey,
    pub new_blacklister: Pubkey
}

#[event] 
pub struct TransferHookUpdated {
    pub config: Pubkey,
    pub old_hook_program: Option<Pubkey>,
    pub new_hook_program: Option<Pubkey>
}

#[event] 
pub struct PausedChanged {
    pub paused: bool
}

#[event] 
pub struct MinterAdded {
    pub config: Pubkey,
    pub minter: Pubkey
}

#[event] 
pub struct MinterRemoved {
    pub config: Pubkey,
    pub minter: Pubkey
}

#[event] 
pub struct FreezerUpdated {
    pub config: Pubkey,
    pub old_freezer: Pubkey,
    pub new_freezer: Pubkey
}

#[event] 
pub struct PauserUpdated {
    pub config: Pubkey,
    pub old_pauser: Pubkey,
    pub new_pauser: Pubkey
}

#[event] 
pub struct MasterAuthorityProposed {
    pub new_authority: Pubkey
}

#[event] 
pub struct MasterAuthorityAccepted {
    pub new_authority: Pubkey }
