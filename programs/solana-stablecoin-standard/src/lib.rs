use anchor_lang::{prelude::*, solana_program::program::invoke};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

mod error;
mod event;
pub mod state;

use crate::{error::*, event::*, state::*};

declare_id!("C78Fk7ZeyGuQV92u3aKJQSeXMn35A9Jrjeyv33UNE4Nw");

#[program]
pub mod solana_stablecoin_standard {
    use super::*;

    // ── Core initialization ──────────────────────────────────────────────────

    /// Initialize a stablecoin. No preset — start with core only.
    /// Attach ComplianceModule or PrivacyModule separately to add capabilities.
    pub fn initialize(ctx: Context<Initialize>, supply_cap: Option<u64>, decimals: u8) -> Result<()> {
        let config = &mut ctx.accounts.config;
        let authority_key = ctx.accounts.authority.key();
        let mint_key = ctx.accounts.mint.key();

        config.master_authority = authority_key;
        config.mint = mint_key;
        config.paused = false;
        config.supply_cap = supply_cap;
        config.decimals = decimals;
        config.bump = ctx.bumps.config;
        config.pending_master_authority = None;
        config.minters = vec![authority_key];
        config.freezer = authority_key;
        config.pauser = authority_key;

        let config_key = config.key();

        // Transfer mint authority to config PDA
        let set_mint_authority_ix = spl_token_2022::instruction::set_authority(
            &ctx.accounts.token_program.key(),
            &mint_key,
            Some(&config_key),
            spl_token_2022::instruction::AuthorityType::MintTokens,
            &authority_key,
            &[],
        )?;
        invoke(
            &set_mint_authority_ix,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.authority.to_account_info(),
            ],
        )?;

        // Transfer freeze authority to config PDA
        let set_freeze_authority_ix = spl_token_2022::instruction::set_authority(
            &ctx.accounts.token_program.key(),
            &mint_key,
            Some(&config_key),
            spl_token_2022::instruction::AuthorityType::FreezeAccount,
            &authority_key,
            &[],
        )?;
        invoke(
            &set_freeze_authority_ix,
            &[
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.authority.to_account_info(),
            ],
        )?;

        emit!(ConfigInitialized { config: config_key, authority: authority_key, mint: mint_key });

        Ok(())
    }

    // ── Module: attach / detach ──────────────────────────────────────────────

    /// Attach the compliance module to this stablecoin.
    /// Enables: blacklist, transfer hook, permanent delegate.
    /// Can be called on any token, any time, by master_authority.
    /// SECURITY: `init` prevents double-attaching — call detach first to reconfigure.
    pub fn attach_compliance_module(
        ctx: Context<AttachComplianceModule>,
        blacklister: Pubkey,
        transfer_hook_program: Option<Pubkey>,
        permanent_delegate: Option<Pubkey>,
    ) -> Result<()> {
        // SECURITY: Reject zero pubkey blacklister — would silently disable blacklist enforcement.
        require!(blacklister != Pubkey::default(), SssError::InvalidAddress);

        if let Some(hook) = transfer_hook_program {
            require!(hook != Pubkey::default(), SssError::InvalidAddress);
        }

        let module = &mut ctx.accounts.compliance_module;
        module.config = ctx.accounts.config.key();
        module.authority = ctx.accounts.authority.key();
        module.blacklister = blacklister;
        module.transfer_hook_program = transfer_hook_program;
        module.permanent_delegate = permanent_delegate;
        module.bump = ctx.bumps.compliance_module;

        emit!(ComplianceModuleAttached { config: ctx.accounts.config.key(), blacklister, transfer_hook_program });

        Ok(())
    }

    /// Detach the compliance module. Immediately disables blacklist enforcement,
    /// transfer hook, and permanent delegate for this token.
    /// Rent returned to master_authority.
    pub fn detach_compliance_module(_ctx: Context<DetachComplianceModule>) -> Result<()> {
        emit!(ComplianceModuleDetached { config: _ctx.accounts.config.key() });
        Ok(())
    }

    /// Attach the privacy module to this stablecoin.
    /// Enables: allowlist gating, confidential transfers (Token-2022).
    pub fn attach_privacy_module(
        ctx: Context<AttachPrivacyModule>,
        allowlist_authority: Pubkey,
        confidential_transfers_enabled: bool,
    ) -> Result<()> {
        require!(allowlist_authority != Pubkey::default(), SssError::InvalidAddress);

        let module = &mut ctx.accounts.privacy_module;
        module.config = ctx.accounts.config.key();
        module.authority = ctx.accounts.authority.key();
        module.allowlist_authority = allowlist_authority;
        module.confidential_transfers_enabled = confidential_transfers_enabled;
        module.bump = ctx.bumps.privacy_module;

        emit!(PrivacyModuleAttached {
            config: ctx.accounts.config.key(),
            allowlist_authority,
            confidential_transfers_enabled,
        });

        Ok(())
    }

    /// Detach the privacy module. Immediately disables allowlist enforcement.
    pub fn detach_privacy_module(_ctx: Context<DetachPrivacyModule>) -> Result<()> {
        emit!(PrivacyModuleDetached { config: _ctx.accounts.config.key() });
        Ok(())
    }

    /// Update the blacklister address on the compliance module.
    pub fn update_blacklister(ctx: Context<UpdateComplianceField>, new_blacklister: Pubkey) -> Result<()> {
        require!(new_blacklister != Pubkey::default(), SssError::InvalidAddress);
        let old = ctx.accounts.compliance_module.blacklister;
        ctx.accounts.compliance_module.blacklister = new_blacklister;
        emit!(BlacklisterUpdated { config: ctx.accounts.config.key(), old_blacklister: old, new_blacklister });
        Ok(())
    }

    /// Update the transfer hook program on the compliance module.
    pub fn update_transfer_hook(ctx: Context<UpdateComplianceField>, new_hook_program: Option<Pubkey>) -> Result<()> {
        if let Some(hook) = new_hook_program {
            require!(hook != Pubkey::default(), SssError::InvalidAddress);
        }
        let old = ctx.accounts.compliance_module.transfer_hook_program;
        ctx.accounts.compliance_module.transfer_hook_program = new_hook_program;
        emit!(TransferHookUpdated { config: ctx.accounts.config.key(), old_hook_program: old, new_hook_program });
        Ok(())
    }

    /// Update the allowlist authority on the privacy module.
    pub fn update_allowlist_authority(ctx: Context<UpdatePrivacyField>, new_authority: Pubkey) -> Result<()> {
        require!(new_authority != Pubkey::default(), SssError::InvalidAddress);
        ctx.accounts.privacy_module.allowlist_authority = new_authority;
        Ok(())
    }

    pub fn blacklist_add(ctx: Context<BlacklistAdd>, reason: String) -> Result<()> {
        // if no compliance module is attached, this instruction cannot be constructed.
        require!(ctx.accounts.target.key() != Pubkey::default(), SssError::InvalidAddress);
        require!(reason.len() <= 128, SssError::ReasonTooLong);

        let entry = &mut ctx.accounts.blacklist_entry;
        entry.blacklister = ctx.accounts.blacklister.key();
        entry.reason = reason.clone();
        entry.timestamp = Clock::get()?.unix_timestamp;
        entry.bump = ctx.bumps.blacklist_entry;

        emit!(AddedToBlacklist {
            config: ctx.accounts.config.key(),
            target: ctx.accounts.target.key(),
            reason,
            blacklister: ctx.accounts.blacklister.key(),
        });
        Ok(())
    }

    pub fn blacklist_remove(ctx: Context<BlacklistRemove>) -> Result<()> {
        emit!(RemovedFromBlacklist {
            config: ctx.accounts.config.key(),
            target: ctx.accounts.target.key(),
            blacklister: ctx.accounts.authority.key(),
        });
        Ok(())
    }

    pub fn allowlist_add(ctx: Context<AllowlistAdd>) -> Result<()> {
        require!(ctx.accounts.wallet.key() != Pubkey::default(), SssError::InvalidAddress);

        let entry = &mut ctx.accounts.allowlist_entry;
        entry.wallet = ctx.accounts.wallet.key();
        entry.approved_by = ctx.accounts.allowlist_authority.key();
        entry.approved_at = Clock::get()?.unix_timestamp;
        entry.bump = ctx.bumps.allowlist_entry;

        emit!(AddedToAllowlist {
            config: ctx.accounts.config.key(),
            wallet: ctx.accounts.wallet.key(),
            approved_by: ctx.accounts.allowlist_authority.key(),
        });
        Ok(())
    }

    pub fn allowlist_remove(ctx: Context<AllowlistRemove>) -> Result<()> {
        emit!(RemovedFromAllowlist { config: ctx.accounts.config.key(), wallet: ctx.accounts.wallet.key() });
        Ok(())
    }

    pub fn mint_tokens(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.minters.contains(&ctx.accounts.minter.key()), SssError::UnauthorizedMinter);
        require!(!config.paused, SssError::MintPaused);

        let new_supply = ctx.accounts.mint.supply.checked_add(amount).ok_or(SssError::Overflow)?;
        if let Some(cap) = config.supply_cap {
            require!(new_supply <= cap, SssError::SupplyCapExceeded);
        }

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin" as &[u8], mint_key.as_ref(), &[config.bump]];
        let signer_seeds = &[&seeds[..]];

        anchor_spl::token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        emit!(TokensMinted {
            mint: mint_key,
            to: ctx.accounts.destination.key(),
            amount,
            minter: ctx.accounts.minter.key(),
        });
        Ok(())
    }

    pub fn burn_tokens(ctx: Context<BurnTokens>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(!config.paused, SssError::BurnPaused);
        require!(config.minters.contains(&ctx.accounts.burner.key()), SssError::UnauthorizedMinter);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin" as &[u8], mint_key.as_ref(), &[config.bump]];
        let signer_seeds = &[&seeds[..]];

        anchor_spl::token_interface::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.from.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        emit!(TokensBurned {
            mint: mint_key,
            from: ctx.accounts.from.key(),
            amount,
            burner: ctx.accounts.burner.key(),
        });
        Ok(())
    }

    /// Transfer with optional compliance and privacy enforcement.
    /// Passes module accounts as UncheckedAccount — existence checked via data_len().
    /// If no compliance module is attached: blacklist checks are skipped.
    /// If no privacy module is attached: allowlist checks are skipped.
    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(!config.paused, SssError::TransfersPaused);

        // data_len() > 0 is the existence check. Seeds constraint ensures the PDA
        // is legitimately derived — it cannot be spoofed by passing an arbitrary account.
        if ctx.accounts.compliance_module.data_len() > 0 {
            // Blacklist check: reject if sender or receiver is blacklisted.
            // Seeds constraints ensure these are the correct PDAs, not attacker-supplied ones.
            if ctx.accounts.sender_blacklist.data_len() > 0 {
                return Err(SssError::SenderBlacklisted.into());
            }
            if ctx.accounts.receiver_blacklist.data_len() > 0 {
                return Err(SssError::ReceiverBlacklisted.into());
            }
        }

        // Privacy checks — only enforced if PrivacyModule is attached.
        if ctx.accounts.privacy_module.data_len() > 0 {
            // Allowlist check: both sender and receiver must be explicitly approved.
            require!(ctx.accounts.sender_allowlist.data_len() > 0, SssError::SenderNotAllowlisted);
            require!(ctx.accounts.receiver_allowlist.data_len() > 0, SssError::ReceiverNotAllowlisted);
        }

        anchor_spl::token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::TransferChecked {
                    from: ctx.accounts.from.to_account_info(),
                    to: ctx.accounts.to.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;

        emit!(TokensTransferred {
            from: ctx.accounts.from.key(),
            to: ctx.accounts.to.key(),
            amount,
            authority: ctx.accounts.authority.key(),
        });
        Ok(())
    }

    /// Seize tokens from a blacklisted wallet. Requires ComplianceModule.
    pub fn seize(ctx: Context<Seize>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(config.minters.contains(&ctx.accounts.seizer.key()), SssError::UnauthorizedSeizer);

        let mint_key = ctx.accounts.mint.key();
        let decimals = ctx.accounts.mint.decimals;
        let seeds = &[b"stablecoin" as &[u8], mint_key.as_ref(), &[config.bump]];

        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.source.key(),
            &mint_key,
            &ctx.accounts.destination.key(),
            &config.key(),
            &[],
            amount,
            decimals,
        )?;
        anchor_lang::solana_program::program::invoke_signed(
            &transfer_ix,
            &[
                ctx.accounts.source.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.destination.to_account_info(),
                ctx.accounts.config.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
            ],
            &[seeds],
        )?;

        emit!(TokensSeized {
            mint: mint_key,
            from: ctx.accounts.source.key(),
            to: ctx.accounts.destination.key(),
            amount,
            seizer: ctx.accounts.seizer.key(),
        });
        Ok(())
    }

    pub fn freeze_account(ctx: Context<FreezeAccount>) -> Result<()> {
        let config = &ctx.accounts.config;
        require_keys_eq!(ctx.accounts.freezer.key(), config.freezer, SssError::UnauthorizedFreezer);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin" as &[u8], mint_key.as_ref(), &[config.bump]];
        let signer_seeds = &[&seeds[..]];

        anchor_spl::token_interface::freeze_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::FreezeAccount {
                account: ctx.accounts.account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ))?;

        emit!(AccountFrozen {
            account: ctx.accounts.account.key(),
            mint: mint_key,
            freezer: ctx.accounts.freezer.key(),
        });
        Ok(())
    }

    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        let config = &ctx.accounts.config;
        require_keys_eq!(ctx.accounts.freezer.key(), config.freezer, SssError::UnauthorizedFreezer);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin" as &[u8], mint_key.as_ref(), &[config.bump]];
        let signer_seeds = &[&seeds[..]];

        anchor_spl::token_interface::thaw_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::ThawAccount {
                account: ctx.accounts.account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ))?;

        emit!(AccountThawed {
            account: ctx.accounts.account.key(),
            mint: mint_key,
            freezer: ctx.accounts.freezer.key(),
        });
        Ok(())
    }

    pub fn update_paused(ctx: Context<UpdatePaused>, paused: bool) -> Result<()> {
        require_keys_eq!(ctx.accounts.authority.key(), ctx.accounts.config.pauser, SssError::UnauthorizedPauser);
        ctx.accounts.config.paused = paused;
        emit!(PausedChanged { paused });
        Ok(())
    }

    pub fn add_minter(ctx: Context<UpdateMinter>, new_minter: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(new_minter != Pubkey::default(), SssError::InvalidAddress);
        require!(!config.minters.contains(&new_minter), SssError::AlreadyMinter);

        require!(config.minters.len() < 10, SssError::TooManyMinters);
        config.minters.push(new_minter);
        emit!(MinterAdded { config: ctx.accounts.config.key(), minter: new_minter });
        Ok(())
    }

    pub fn remove_minter(ctx: Context<UpdateMinter>, minter: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        let pos = config.minters.iter().position(|m| *m == minter).ok_or(SssError::MinterNotFound)?;
        config.minters.remove(pos);
        emit!(MinterRemoved { config: ctx.accounts.config.key(), minter });
        Ok(())
    }

    pub fn update_freezer(ctx: Context<UpdateFreezer>, new_freezer: Pubkey) -> Result<()> {
        require!(new_freezer != Pubkey::default(), SssError::InvalidAddress);
        let old = ctx.accounts.config.freezer;
        ctx.accounts.config.freezer = new_freezer;
        emit!(FreezerUpdated { config: ctx.accounts.config.key(), old_freezer: old, new_freezer });
        Ok(())
    }

    pub fn update_pauser(ctx: Context<UpdatePauser>, new_pauser: Pubkey) -> Result<()> {
        require!(new_pauser != Pubkey::default(), SssError::InvalidAddress);
        let old = ctx.accounts.config.pauser;
        ctx.accounts.config.pauser = new_pauser;
        emit!(PauserUpdated { config: ctx.accounts.config.key(), old_pauser: old, new_pauser });
        Ok(())
    }

    pub fn update_supply_cap(ctx: Context<UpdateSupplyCap>, new_cap: Option<u64>) -> Result<()> {
        ctx.accounts.config.supply_cap = new_cap;
        Ok(())
    }

    pub fn propose_master_authority(ctx: Context<ProposeMasterAuthority>, new_authority: Pubkey) -> Result<()> {
        require!(new_authority != Pubkey::default(), SssError::InvalidAddress);
        ctx.accounts.config.pending_master_authority = Some(new_authority);
        emit!(MasterAuthorityProposed { new_authority });
        Ok(())
    }

    pub fn accept_master_authority(ctx: Context<AcceptMasterAuthority>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(
            config.pending_master_authority == Some(ctx.accounts.new_authority.key()),
            SssError::NoPendingTransfer
        );
        config.master_authority = ctx.accounts.new_authority.key();
        config.pending_master_authority = None;
        emit!(MasterAuthorityAccepted { new_authority: ctx.accounts.new_authority.key() });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + StablecoinConfig::INIT_SPACE,
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AttachComplianceModule<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + ComplianceModule::INIT_SPACE,
        // SECURITY: Seeds bind this module exclusively to this config — no sharing.
        seeds = [b"compliance", config.key().as_ref()],
        bump,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        // SECURITY: has_one ensures only master_authority can attach modules.
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DetachComplianceModule<'info> {
    #[account(
        mut,
        // SECURITY: `close` zeroes data, transfers lamports, reassigns to System Program.
        close = authority,
        seeds = [b"compliance", config.key().as_ref()],
        bump = compliance_module.bump,
        // SECURITY: has_one verifies this module belongs to this config.
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AttachPrivacyModule<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + PrivacyModule::INIT_SPACE,
        seeds = [b"privacy", config.key().as_ref()],
        bump,
    )]
    pub privacy_module: Account<'info, PrivacyModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DetachPrivacyModule<'info> {
    #[account(
        mut,
        close = authority,
        seeds = [b"privacy", config.key().as_ref()],
        bump = privacy_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub privacy_module: Account<'info, PrivacyModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateComplianceField<'info> {
    #[account(
        mut,
        seeds = [b"compliance", config.key().as_ref()],
        bump = compliance_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdatePrivacyField<'info> {
    #[account(
        mut,
        seeds = [b"privacy", config.key().as_ref()],
        bump = privacy_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub privacy_module: Account<'info, PrivacyModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct BlacklistAdd<'info> {
    #[account(
        init,
        payer = blacklister,
        space = 8 + BlacklistEntry::INIT_SPACE,
        seeds = [b"blacklist", config.key().as_ref(), target.key().as_ref()],
        bump,
    )]
    pub blacklist_entry: Account<'info, BlacklistEntry>,
    // SECURITY: Requiring ComplianceModule here makes blacklist_add impossible
    // without an attached module — the instruction will fail at account loading.
    #[account(
        seeds = [b"compliance", config.key().as_ref()],
        bump = compliance_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
        // SECURITY: has_one verifies blacklister matches the stored address in the module.
        constraint = compliance_module.blacklister == blacklister.key() @ SssError::UnauthorizedBlacklister,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub blacklister: Signer<'info>,
    pub target: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BlacklistRemove<'info> {
    #[account(
        mut,
        close = authority,
        seeds = [b"blacklist", config.key().as_ref(), target.key().as_ref()],
        bump = blacklist_entry.bump,
    )]
    pub blacklist_entry: Account<'info, BlacklistEntry>,
    #[account(
        seeds = [b"compliance", config.key().as_ref()],
        bump = compliance_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
    pub target: SystemAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AllowlistAdd<'info> {
    #[account(
        init,
        payer = allowlist_authority,
        space = 8 + AllowlistEntry::INIT_SPACE,
        seeds = [b"allowlist", privacy_module.key().as_ref(), wallet.key().as_ref()],
        bump,
    )]
    pub allowlist_entry: Account<'info, AllowlistEntry>,
    // SECURITY: Requiring PrivacyModule ensures allowlist_add is impossible without module.
    #[account(
        seeds = [b"privacy", config.key().as_ref()],
        bump = privacy_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
        constraint = privacy_module.allowlist_authority == allowlist_authority.key() @ SssError::UnauthorizedAllowlistAuthority,
    )]
    pub privacy_module: Account<'info, PrivacyModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub allowlist_authority: Signer<'info>,
    pub wallet: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AllowlistRemove<'info> {
    #[account(
        mut,
        close = allowlist_authority,
        seeds = [b"allowlist", privacy_module.key().as_ref(), wallet.key().as_ref()],
        bump = allowlist_entry.bump,
    )]
    pub allowlist_entry: Account<'info, AllowlistEntry>,
    #[account(
        seeds = [b"privacy", config.key().as_ref()],
        bump = privacy_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
        constraint = privacy_module.allowlist_authority == allowlist_authority.key() @ SssError::UnauthorizedAllowlistAuthority,
    )]
    pub privacy_module: Account<'info, PrivacyModule>,
    #[account(
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub allowlist_authority: Signer<'info>,
    pub wallet: SystemAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,

    /// CHECK: ComplianceModule PDA. Existence (data_len > 0) enables compliance checks.
    /// Seeds constraint proves this is the real module PDA for this config — not spoofed.
    #[account(seeds = [b"compliance", config.key().as_ref()], bump)]
    pub compliance_module: UncheckedAccount<'info>,

    /// CHECK: Blacklist entry for the sender's wallet. Existence means sender is blacklisted.
    /// Seeds derived from (config, from.owner) — cannot be substituted.
    #[account(seeds = [b"blacklist", config.key().as_ref(), from.owner.as_ref()], bump)]
    pub sender_blacklist: UncheckedAccount<'info>,

    /// CHECK: Blacklist entry for the receiver's wallet. Existence means receiver is blacklisted.
    #[account(seeds = [b"blacklist", config.key().as_ref(), to.owner.as_ref()], bump)]
    pub receiver_blacklist: UncheckedAccount<'info>,

    /// CHECK: PrivacyModule PDA. Existence enables allowlist enforcement.
    #[account(seeds = [b"privacy", config.key().as_ref()], bump)]
    pub privacy_module: UncheckedAccount<'info>,

    /// CHECK: Allowlist entry for the sender. Existence means sender is approved.
    #[account(seeds = [b"allowlist", privacy_module.key().as_ref(), from.owner.as_ref()], bump)]
    pub sender_allowlist: UncheckedAccount<'info>,

    /// CHECK: Allowlist entry for the receiver. Existence means receiver is approved.
    #[account(seeds = [b"allowlist", privacy_module.key().as_ref(), to.owner.as_ref()], bump)]
    pub receiver_allowlist: UncheckedAccount<'info>,

    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, constraint = from.key() != to.key() @ SssError::SameAccount)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Seize<'info> {
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,
    // SECURITY: ComplianceModule required — seize is only possible with compliance attached.
    #[account(
        seeds = [b"compliance", config.key().as_ref()],
        bump = compliance_module.bump,
        has_one = config @ SssError::ModuleConfigMismatch,
    )]
    pub compliance_module: Account<'info, ComplianceModule>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    // SECURITY: Source must have a blacklist entry — can only seize from blacklisted wallets.
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), source.owner.as_ref()],
        bump = source_blacklist.bump,
    )]
    pub source_blacklist: Account<'info, BlacklistEntry>,
    // SECURITY: Source and destination must differ — no seize-to-self.
    #[account(mut, constraint = source.key() != destination.key() @ SssError::SameAccount)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub seizer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(seeds = [b"stablecoin", mint.key().as_ref()], bump = config.bump)]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut, address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub minter: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct BurnTokens<'info> {
    #[account(seeds = [b"stablecoin", mint.key().as_ref()], bump = config.bump)]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut, address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub burner: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct FreezeAccount<'info> {
    #[account(seeds = [b"stablecoin", mint.key().as_ref()], bump = config.bump)]
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub account: InterfaceAccount<'info, TokenAccount>,
    pub freezer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ThawAccount<'info> {
    #[account(seeds = [b"stablecoin", mint.key().as_ref()], bump = config.bump)]
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub account: InterfaceAccount<'info, TokenAccount>,
    pub freezer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct UpdatePaused<'info> {
    #[account(mut, seeds = [b"stablecoin", config.mint.as_ref()], bump = config.bump)]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMinter<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFreezer<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdatePauser<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateSupplyCap<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProposeMasterAuthority<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
        has_one = master_authority @ SssError::Unauthorized,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub master_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct AcceptMasterAuthority<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub new_authority: Signer<'info>,
}
