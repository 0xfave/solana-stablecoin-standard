use anchor_lang::prelude::*;
use anchor_lang::solana_program::account_info::AccountInfo;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

declare_id!("Ak5zCGByVQ972WfccBAxR67zZambk5KqUvfEfksUMXr6");

use crate::state::{BlacklistEntry, StablecoinConfig};

pub mod error;
pub mod event;
pub mod state;

use crate::{error::*, event::*};

#[program]
pub mod solana_stablecoin_standard {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, preset: u8, supply_cap: Option<u64>, decimals: u8) -> Result<()> {
        let config = &mut ctx.accounts.config;
        let authority_key = ctx.accounts.authority.key();
        let mint_key = ctx.accounts.mint.key();
        config.master_authority = authority_key;
        config.mint = mint_key;
        config.preset = preset;
        config.paused = false;
        config.supply_cap = supply_cap;
        config.transfer_hook_program = None;
        config.decimals = decimals;
        config.bump = ctx.bumps.config;
        config.pending_master_authority = None;
        config.minters = vec![authority_key];
        config.freezer = authority_key;
        config.pauser = authority_key;
        config.blacklister = authority_key;

        let config_key = ctx.accounts.config.key();
        let mint_key = ctx.accounts.mint.key();

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

        emit!(ConfigInitialized {
            config: ctx.accounts.config.key(),
            authority: authority_key,
            mint: mint_key,
            preset,
        });

        Ok(())
    }

    pub fn update_paused(ctx: Context<UpdatePaused>, paused: bool) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.pauser, StablecoinError::UnauthorizedPauser);
        config.paused = paused;
        emit!(PausedChanged { paused });
        Ok(())
    }

    pub fn update_transfer_hook(ctx: Context<UpdateTransferHook>, new_hook_program: Option<Pubkey>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        let old_hook = config.transfer_hook_program;
        config.transfer_hook_program = new_hook_program;
        emit!(TransferHookUpdated { config: ctx.accounts.config.key(), old_hook_program: old_hook, new_hook_program });
        Ok(())
    }

    pub fn add_minter(ctx: Context<UpdateMinter>, new_minter: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        require!(!config.minters.contains(&new_minter), StablecoinError::AlreadyMinter);
        require!(config.minters.len() < 10, StablecoinError::TooManyMinters);
        config.minters.push(new_minter);
        emit!(MinterAdded { config: ctx.accounts.config.key(), minter: new_minter });
        Ok(())
    }

    pub fn remove_minter(ctx: Context<UpdateMinter>, minter: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        let pos = config.minters.iter().position(|m| *m == minter).ok_or(StablecoinError::MinterNotFound)?;
        config.minters.remove(pos);
        emit!(MinterRemoved { config: ctx.accounts.config.key(), minter });
        Ok(())
    }

    pub fn update_freezer(ctx: Context<UpdateFreezer>, new_freezer: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        let old_freezer = config.freezer;
        config.freezer = new_freezer;
        emit!(FreezerUpdated { config: ctx.accounts.config.key(), old_freezer, new_freezer });
        Ok(())
    }

    pub fn update_pauser(ctx: Context<UpdatePauser>, new_pauser: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        let old_pauser = config.pauser;
        config.pauser = new_pauser;
        emit!(PauserUpdated { config: ctx.accounts.config.key(), old_pauser, new_pauser });
        Ok(())
    }

    pub fn update_blacklister(ctx: Context<UpdateBlacklister>, new_blacklister: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        let old_blacklister = config.blacklister;
        config.blacklister = new_blacklister;
        emit!(BlacklisterUpdated { config: ctx.accounts.config.key(), old_blacklister, new_blacklister });
        Ok(())
    }

    pub fn mint(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.minters.contains(&ctx.accounts.minter.key()), StablecoinError::UnauthorizedMinter);
        require!(!config.paused, StablecoinError::MintPaused);
        let new_supply = ctx.accounts.mint.supply.checked_add(amount).ok_or(StablecoinError::Overflow)?;
        if let Some(cap) = config.supply_cap {
            require!(new_supply <= cap, StablecoinError::Overflow);
        }

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin", mint_key.as_ref(), &[config.bump]];
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
            mint: ctx.accounts.mint.key(),
            to: ctx.accounts.destination.key(),
            amount,
            minter: ctx.accounts.minter.key(),
        });
        Ok(())
    }

    pub fn burn(ctx: Context<Burn>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(!config.paused, StablecoinError::BurnPaused);
        require!(config.minters.contains(&ctx.accounts.burner.key()), StablecoinError::UnauthorizedMinter);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin", mint_key.as_ref(), &[config.bump]];
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
            mint: ctx.accounts.mint.key(),
            from: ctx.accounts.from.key(),
            amount,
            burner: ctx.accounts.burner.key(),
        });
        Ok(())
    }

    pub fn seize(ctx: Context<Seize>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        require!(config.minters.contains(&ctx.accounts.seizer.key()), StablecoinError::UnauthorizedSeizer);

        let source_blacklist_key = ctx.accounts.source_blacklist.key();
        let (expected_blacklist, _) = Pubkey::find_program_address(
            &[b"blacklist", config.key().as_ref(), ctx.accounts.source.owner.as_ref()],
            &ID,
        );
        require_keys_eq!(source_blacklist_key, expected_blacklist, StablecoinError::InvalidBlacklistAccount);

        let mint_key = ctx.accounts.mint.key();
        let decimals = ctx.accounts.mint.decimals;

        let seeds = &[b"stablecoin", mint_key.as_ref(), &[config.bump]];

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

        invoke_signed(
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
        require_keys_eq!(ctx.accounts.freezer.key(), config.freezer, StablecoinError::UnauthorizedFreezer);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin", mint_key.as_ref(), &[config.bump]];
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
            mint: ctx.accounts.mint.key(),
            freezer: ctx.accounts.freezer.key(),
        });
        Ok(())
    }

    pub fn thaw_account(ctx: Context<ThawAccount>) -> Result<()> {
        let config = &ctx.accounts.config;
        require_keys_eq!(ctx.accounts.freezer.key(), config.freezer, StablecoinError::UnauthorizedFreezer);

        let mint_key = ctx.accounts.mint.key();
        let seeds = &[b"stablecoin", mint_key.as_ref(), &[config.bump]];
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
            mint: ctx.accounts.mint.key(),
            freezer: ctx.accounts.freezer.key(),
        });
        Ok(())
    }

    pub fn blacklist_add(ctx: Context<BlacklistAdd>, reason: String) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        require_keys_eq!(ctx.accounts.blacklister.key(), config.blacklister, StablecoinError::UnauthorizedBlacklister);
        require!(ctx.accounts.target.key() != Pubkey::default(), StablecoinError::BlacklistZeroAddress);
        let entry = &mut ctx.accounts.blacklist_entry;
        entry.blacklister = ctx.accounts.blacklister.key();
        entry.reason = reason;
        entry.timestamp = Clock::get()?.unix_timestamp;
        entry.bump = ctx.bumps.blacklist_entry;
        emit!(AddedToBlacklist {
            config: ctx.accounts.config.key(),
            target: ctx.accounts.target.key(),
            reason: entry.reason.clone(),
            blacklister: ctx.accounts.blacklister.key(),
        });
        Ok(())
    }

    pub fn blacklist_remove(ctx: Context<BlacklistRemove>) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        require!(ctx.accounts.blacklist_entry.to_account_info().data_len() > 0, StablecoinError::NotBlacklisted);
        let target = ctx.accounts.blacklist_entry.key();
        ctx.accounts.blacklist_entry.close(ctx.accounts.destination.to_account_info())?;
        emit!(RemovedFromBlacklist {
            config: ctx.accounts.config.key(),
            target,
            blacklister: ctx.accounts.authority.key(),
        });
        Ok(())
    }

    pub fn transfer(ctx: Context<Transfer>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        if ctx.accounts.sender_blacklist.data_len() > 0 {
            return Err(StablecoinError::SenderBlacklisted.into());
        }
        if ctx.accounts.receiver_blacklist.data_len() > 0 {
            return Err(StablecoinError::ReceiverBlacklisted.into());
        }

        if config.preset == 1 {
            require!(config.transfer_hook_program.is_some(), StablecoinError::TransferHookRequired);
        }

        require!(!config.paused, StablecoinError::TransfersPaused);

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

    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        let config = &ctx.accounts.config;
        require!(config.preset == 1, StablecoinError::NotCompliantMode);
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        msg!("ExtraAccountMetaList initialized for mint: {}", ctx.accounts.mint.key());
        Ok(())
    }

    pub fn propose_master_authority(ctx: Context<ProposeMasterAuthority>, new_authority: Pubkey) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require_keys_eq!(ctx.accounts.authority.key(), config.master_authority, StablecoinError::Unauthorized);
        config.pending_master_authority = Some(new_authority);
        emit!(MasterAuthorityProposed { new_authority });
        Ok(())
    }

    pub fn accept_master_authority(ctx: Context<AcceptMasterAuthority>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        require!(
            config.pending_master_authority == Some(ctx.accounts.new_authority.key()),
            StablecoinError::NoPendingTransfer
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
        space = StablecoinConfig::INIT_SPACE,
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
pub struct UpdatePaused<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateTransferHook<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateMinter<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFreezer<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdatePauser<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateBlacklister<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    pub minter: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Burn<'info> {
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub burner: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Seize<'info> {
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut, constraint = source.key() != destination.key() @ StablecoinError::SameAccount)]
    pub source: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), source.owner.as_ref()],
        bump,
    )]
    pub source_blacklist: Account<'info, BlacklistEntry>,
    pub seizer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct FreezeAccount<'info> {
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
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
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(address = config.mint)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub account: InterfaceAccount<'info, TokenAccount>,
    pub freezer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct BlacklistAdd<'info> {
    #[account(
        init,
        payer = blacklister,
        space = BlacklistEntry::INIT_SPACE,
        seeds = [b"blacklist", config.key().as_ref(), target.key().as_ref()],
        bump
    )]
    pub blacklist_entry: Account<'info, BlacklistEntry>,
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
        close = destination,
        seeds = [b"blacklist", config.key().as_ref(), target.key().as_ref()],
        bump = blacklist_entry.bump
    )]
    pub blacklist_entry: Account<'info, BlacklistEntry>,
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
    pub target: SystemAccount<'info>,
    #[account(mut)]
    pub destination: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    pub config: Account<'info, StablecoinConfig>,
    /// CHECK: Validated by seeds constraint, account data length checked in instruction
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), from.owner.as_ref()],
        bump,
    )]
    pub sender_blacklist: UncheckedAccount<'info>,
    /// CHECK: Validated by seeds constraint, account data length checked in instruction
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), to.owner.as_ref()],
        bump,
    )]
    pub receiver_blacklist: UncheckedAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(mut, constraint = from.key() != to.key() @ StablecoinError::SameAccount)]
    pub from: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProposeMasterAuthority<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>, // must be current master_authority
}

#[derive(Accounts)]
pub struct AcceptMasterAuthority<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    /// The new authority accepting the transfer – must match pending_master_authority
    pub new_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    #[account(
        mut,
        seeds = [b"stablecoin", config.mint.as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    pub authority: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
}
