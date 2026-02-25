use anchor_lang::prelude::*;

declare_id!("3cJyL8kQwwKHoUPs3MCPivExBdnFt1y5XipxChW2uKXS");
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::state::{Blacklist, StablecoinConfig};

pub mod error;
pub mod event;
pub mod state;

use crate::error::*;
use crate::event::*;

#[program]
pub mod solana_stablecoin_standard {
    use super::*;

    /// Initialize the stablecoin config (SSS-1 or SSS-2)
    /// - SSS-1: basic mint + freeze + metadata
    /// - SSS-2: adds permanent delegate + optional transfer hook + blacklist support
    pub fn initialize(ctx: Context<Initialize>, is_compliant: bool) -> Result<()> {
        let config_key = ctx.accounts.config.key();

        let config = &mut ctx.accounts.config;
        config.owner = ctx.accounts.owner.key();
        config.mint = ctx.accounts.mint.key();

        // All roles default to the initializer (owner) for safety and simplicity
        config.minter = ctx.accounts.owner.key();
        config.freezer = ctx.accounts.owner.key();
        config.pauser = ctx.accounts.owner.key();

        if is_compliant {
            // SSS-2 compliant mode: enable blacklist and permanent delegate
            config.blacklister = ctx.accounts.owner.key();
            config.permanent_delegate = Some(ctx.accounts.owner.key());
            // Transfer hook is optional and set later via update_transfer_hook
            config.transfer_hook_program = None;
        } else {
            // SSS-1: no compliance extensions
            config.blacklister = Pubkey::default(); // unused
            config.permanent_delegate = None;
            config.transfer_hook_program = None;
        }

        config.bump = ctx.bumps.config;

        emit!(ConfigInitialized {
            config: config_key,
            owner: config.owner,
            mint: config.mint,
            is_compliant
        });

        Ok(())
    }

    /// Update roles (minter, freezer, pauser, blacklister)
    /// Only callable by current owner
    /// Only available in compliant (SSS-2) mode
    pub fn update_roles(
        ctx: Context<UpdateRoles>,
        new_minter: Option<Pubkey>,
        new_freezer: Option<Pubkey>,
        new_pauser: Option<Pubkey>,
        new_blacklister: Option<Pubkey>,
    ) -> Result<()> {
        let config_key = ctx.accounts.config.key(); // capture key first (avoids borrow error)

        let config = &mut ctx.accounts.config;

        // Only owner can update
        require_keys_eq!(
            ctx.accounts.owner.key(),
            config.owner,
            StablecoinError::Unauthorized
        );

        // Only allowed in SSS-2 mode
        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        let old_minter = config.minter;
        let old_freezer = config.freezer;
        let old_pauser = config.pauser;
        let old_blacklister = config.blacklister;

        if let Some(minter) = new_minter {
            config.minter = minter;
        }
        if let Some(freezer) = new_freezer {
            config.freezer = freezer;
        }
        if let Some(pauser) = new_pauser {
            config.pauser = pauser;
        }
        if let Some(blacklister) = new_blacklister {
            config.blacklister = blacklister;
        }

        emit!(RolesUpdated {
            config: config_key,
            old_minter,
            new_minter: config.minter,
            old_freezer,
            new_freezer: config.freezer,
            old_pauser,
            new_pauser: config.pauser,
            old_blacklister,
            new_blacklister: config.blacklister,
        });

        Ok(())
    }

    /// Update the transfer hook program address (SSS-2 only, owner only)
    pub fn update_transfer_hook(
        ctx: Context<UpdateTransferHook>,
        new_hook_program: Option<Pubkey>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;

        require_keys_eq!(
            ctx.accounts.owner.key(),
            config.owner,
            StablecoinError::Unauthorized
        );

        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        let old_hook = config.transfer_hook_program;
        config.transfer_hook_program = new_hook_program;

        emit!(TransferHookUpdated {
            config: ctx.accounts.config.key(),
            old_hook_program: old_hook,
            new_hook_program,
        });

        Ok(())
    }

    pub fn mint(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require_keys_eq!(
            ctx.accounts.minter.key(),
            config.minter,
            StablecoinError::UnauthorizedMinter
        );

        // Optional: add pause check if you implement mint pausing
        // require!(!config.mint_paused, StablecoinError::MintPaused);

        anchor_spl::token_interface::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.minter.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(TokensMinted {
            mint: ctx.accounts.mint.key(),
            to: ctx.accounts.destination.key(),
            amount,
            minter: ctx.accounts.minter.key(),
        });

        Ok(())
    }

    /// Burn tokens from an account
    /// Only callable by the minter
    pub fn burn(ctx: Context<Burn>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require_keys_eq!(
            ctx.accounts.burner.key(),
            config.minter,
            StablecoinError::UnauthorizedMinter
        );

        anchor_spl::token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.from.to_account_info(),
                    authority: ctx.accounts.burner.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(TokensBurned {
            mint: ctx.accounts.mint.key(),
            from: ctx.accounts.from.key(),
            amount,
            burner: ctx.accounts.burner.key(),
        });

        Ok(())
    }

    /// Freeze a token account
    /// Only callable by the freezer
    pub fn freeze_account(ctx: Context<FreezeAccount>) -> Result<()> {
        let config = &ctx.accounts.config;

        require_keys_eq!(
            ctx.accounts.freezer.key(),
            config.freezer,
            StablecoinError::UnauthorizedFreezer
        );

        anchor_spl::token_interface::freeze_account(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::FreezeAccount {
                account: ctx.accounts.account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.freezer.to_account_info(),
            },
        ))?;

        emit!(AccountFrozen {
            account: ctx.accounts.account.key(),
            mint: ctx.accounts.mint.key(),
            freezer: ctx.accounts.freezer.key(),
        });

        Ok(())
    }

    /// Thaw (unfreeze) a token account
    /// Only callable by the freezer
    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        let config = &ctx.accounts.config;

        require_keys_eq!(
            ctx.accounts.freezer.key(),
            config.freezer,
            StablecoinError::UnauthorizedFreezer
        );

        anchor_spl::token_interface::thaw_account(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::ThawAccount {
                account: ctx.accounts.account.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
                authority: ctx.accounts.freezer.to_account_info(),
            },
        ))?;

        emit!(AccountThawed {
            account: ctx.accounts.account.key(),
            mint: ctx.accounts.mint.key(),
            freezer: ctx.accounts.freezer.key(),
        });

        Ok(())
    }

    /// Add an address to the blacklist (SSS-2 only)
    /// Only callable by the blacklister
    pub fn blacklist_add(ctx: Context<BlacklistAdd>, reason: String) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        require_keys_eq!(
            ctx.accounts.blacklister.key(),
            config.blacklister,
            StablecoinError::UnauthorizedBlacklister
        );

        let blacklist = &mut ctx.accounts.blacklist;
        require!(
            !blacklist.entries.contains(&ctx.accounts.target.key()),
            StablecoinError::AlreadyBlacklisted
        );

        blacklist.entries.push(ctx.accounts.target.key());

        emit!(AddedToBlacklist {
            config: ctx.accounts.config.key(),
            target: ctx.accounts.target.key(),
            reason,
            blacklister: ctx.accounts.blacklister.key(),
        });

        Ok(())
    }

    /// Remove an address from the blacklist (SSS-2 only)
    /// Only callable by the blacklister
    pub fn blacklist_remove(ctx: Context<BlacklistRemove>) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        require_keys_eq!(
            ctx.accounts.blacklister.key(),
            config.blacklister,
            StablecoinError::UnauthorizedBlacklister
        );

        let blacklist = &mut ctx.accounts.blacklist;
        let target_key = ctx.accounts.target.key();

        require!(
            blacklist.entries.contains(&target_key),
            StablecoinError::NotBlacklisted
        );

        blacklist.entries.retain(|&x| x != target_key);

        emit!(RemovedFromBlacklist {
            config: ctx.accounts.config.key(),
            target: target_key,
            blacklister: ctx.accounts.blacklister.key(),
        });

        Ok(())
    }

    /// Seize tokens from a blacklisted account (SSS-2 only)
    /// Only callable by the permanent delegate
    pub fn seize(ctx: Context<Seize>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        require_keys_eq!(
            ctx.accounts.seizer.key(),
            config.permanent_delegate.unwrap(),
            StablecoinError::UnauthorizedSeizer
        );

        let blacklist = &ctx.accounts.blacklist;
        require!(
            blacklist.entries.contains(&ctx.accounts.from.owner),
            StablecoinError::NotBlacklisted
        );

        anchor_spl::token_interface::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token_interface::Transfer {
                    from: ctx.accounts.from.to_account_info(),
                    to: ctx.accounts.to.to_account_info(),
                    authority: ctx.accounts.seizer.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(TokensSeized {
            from: ctx.accounts.from.key(),
            to: ctx.accounts.to.key(),
            amount,
            seizer: ctx.accounts.seizer.key(),
        });

        Ok(())
    }

    /// Initialize the blacklist account (SSS-2 only)
    pub fn init_blacklist(ctx: Context<InitBlacklist>) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(
            config.permanent_delegate.is_some(),
            StablecoinError::NotCompliantMode
        );

        require_keys_eq!(
            ctx.accounts.owner.key(),
            config.owner,
            StablecoinError::Unauthorized
        );

        let blacklist = &mut ctx.accounts.blacklist;
        blacklist.entries = vec![];
        blacklist.bump = ctx.bumps.blacklist;

        emit!(BlacklistInitialized {
            config: ctx.accounts.config.key(),
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = owner,
        space = StablecoinConfig::INIT_SPACE,
        seeds = [b"config", mint.key().as_ref()],
        bump
    )]
    pub config: Account<'info, StablecoinConfig>,

    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateRoles<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        mut,
        has_one = owner @ StablecoinError::Unauthorized,
        seeds = [b"config", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
}

#[derive(Accounts)]
pub struct UpdateTransferHook<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner @ StablecoinError::Unauthorized,
        seeds = [b"config", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub minter: Signer<'info>, // minter role
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Burn<'info> {
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub burner: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct FreezeAccount<'info> {
    pub config: Account<'info, StablecoinConfig>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub account: InterfaceAccount<'info, TokenAccount>,
    pub freezer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ThawAccount<'info> {
    pub config: Account<'info, StablecoinConfig>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub account: InterfaceAccount<'info, TokenAccount>,
    pub freezer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct InitBlacklist<'info> {
    #[account(
        init,
        payer = owner,
        space = Blacklist::INIT_SPACE,
        seeds = [b"blacklist", config.key().as_ref()],
        bump
    )]
    pub blacklist: Account<'info, Blacklist>,
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BlacklistAdd<'info> {
    #[account(
        mut,
        seeds = [b"blacklist", config.key().as_ref()],
        bump = blacklist.bump
    )]
    pub blacklist: Account<'info, Blacklist>,
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub blacklister: Signer<'info>,
    pub target: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct BlacklistRemove<'info> {
    #[account(
        mut,
        seeds = [b"blacklist", config.key().as_ref()],
        bump = blacklist.bump
    )]
    pub blacklist: Account<'info, Blacklist>,
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub blacklister: Signer<'info>,
    pub target: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct Seize<'info> {
    pub config: Account<'info, StablecoinConfig>,
    #[account(
        seeds = [b"blacklist", config.key().as_ref()],
        bump = blacklist.bump
    )]
    pub blacklist: Account<'info, Blacklist>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub seizer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}
