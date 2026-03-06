use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct StablecoinConfig {
    pub master_authority: Pubkey,                 // 32
    pub mint: Pubkey,                             // 32
    pub paused: bool,                             // 1
    pub supply_cap: Option<u64>,                  // 9
    pub decimals: u8,                             // 1
    pub bump: u8,                                 // 1
    pub pending_master_authority: Option<Pubkey>, // 33
    #[max_len(10)]
    pub minters: Vec<Pubkey>,   // 4 + 10*32 = 324
    pub freezer: Pubkey,                          // 32
    pub pauser: Pubkey,                           // 32
}

/// Optional compliance module. Attach to enable blacklist, transfer hook,
/// and permanent delegate. Detach to remove all compliance enforcement.
/// PDA seeds: [b"compliance", config.key()]
#[account]
#[derive(InitSpace)]
pub struct ComplianceModule {
    pub config: Pubkey,                        // 32 — back-ref to StablecoinConfig
    pub authority: Pubkey,                     // 32 — who attached this module (master_authority at attach time)
    pub blacklister: Pubkey,                   // 32
    pub transfer_hook_program: Option<Pubkey>, // 33
    pub permanent_delegate: Option<Pubkey>,    // 33
    pub bump: u8,                              // 1
}

/// Optional privacy module. Attach to enable allowlist gating
/// and confidential transfers (Token-2022 extension).
/// PDA seeds: [b"privacy", config.key()]
#[account]
#[derive(InitSpace)]
pub struct PrivacyModule {
    pub config: Pubkey,                       // 32
    pub authority: Pubkey,                    // 32
    pub allowlist_authority: Pubkey,          // 32 — who can add/remove allowlist entries
    pub confidential_transfers_enabled: bool, // 1
    pub bump: u8,                             // 1
}

/// Per-wallet blacklist entry. Requires ComplianceModule to exist.
/// PDA seeds: [b"blacklist", config.key(), wallet.key()]
#[account]
#[derive(InitSpace)]
pub struct BlacklistEntry {
    pub blacklister: Pubkey, // 32 — who added this entry
    #[max_len(128)]
    pub reason: String, // 4 + 128
    pub timestamp: i64,      // 8
    pub bump: u8,            // 1
}

/// Per-wallet allowlist entry. Requires PrivacyModule to exist.
/// PDA seeds: [b"allowlist", privacy_module.key(), wallet.key()]
#[account]
#[derive(InitSpace)]
pub struct AllowlistEntry {
    pub wallet: Pubkey,      // 32
    pub approved_by: Pubkey, // 32
    pub approved_at: i64,    // 8
    pub bump: u8,            // 1
}
