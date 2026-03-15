use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
use solana_stablecoin_standard::ID as SSS_PROGRAM_ID;
use spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList};
use spl_transfer_hook_interface::instruction::ExecuteInstruction;
use spl_transfer_hook_interface::instruction::TransferHookInstruction;

declare_id!("2fLexdN1nyTkWNcnagCSbVKUZ262d8WWAzeQUdjoEt88");

pub mod state;

#[error_code]
pub enum ComplianceError {
    #[msg("Transfers are paused")]
    TransfersPaused,
    #[msg("Sender is blacklisted")]
    SenderBlacklisted,
    #[msg("Receiver is blacklisted")]
    ReceiverBlacklisted,
    #[msg("Invalid instruction")]
    InvalidInstruction,
}

fn verify_config(config: &UncheckedAccount, mint: &Pubkey) -> Result<()> {
    let (expected_config, _) = Pubkey::find_program_address(&[b"stablecoin", mint.as_ref()], &SSS_PROGRAM_ID);
    require_keys_eq!(config.key(), expected_config, ComplianceError::InvalidInstruction);
    require_eq!(*config.owner, SSS_PROGRAM_ID, ComplianceError::InvalidInstruction);
    Ok(())
}

fn enforce_blacklist(
    config_key: &Pubkey,
    source_owner: &Pubkey,
    destination_owner: &Pubkey,
    source_blacklist: &AccountInfo,
    destination_blacklist: &AccountInfo,
    paused: bool,
) -> Result<()> {
    require!(!paused, ComplianceError::TransfersPaused);

    // MUST use main SSS program ID — blacklist PDAs are owned by the main program
    let blacklist_seed = b"blacklist";

    let source_blacklist_key = Pubkey::try_find_program_address(
        &[blacklist_seed, config_key.as_ref(), source_owner.as_ref()],
        &SSS_PROGRAM_ID,
    )
    .map(|(key, _)| key);

    if let Some(key) = source_blacklist_key {
        if *source_blacklist.key == key && source_blacklist.data_len() > 0 {
            return Err(ComplianceError::SenderBlacklisted.into());
        }
    }

    let dest_blacklist_key = Pubkey::try_find_program_address(
        &[blacklist_seed, config_key.as_ref(), destination_owner.as_ref()],
        &SSS_PROGRAM_ID,
    )
    .map(|(key, _)| key);

    if let Some(key) = dest_blacklist_key {
        if *destination_blacklist.key == key && destination_blacklist.data_len() > 0 {
            return Err(ComplianceError::ReceiverBlacklisted.into());
        }
    }

    Ok(())
}

#[program]
pub mod sss_compliance_hook {
    use super::*;

    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        verify_config(&ctx.accounts.config, &ctx.accounts.mint.key())?;

        // Store config PDA as extra account so Token-2022 passes it to fallback
        let account_metas = vec![ExtraAccountMeta::new_with_pubkey(&ctx.accounts.config.key(), false, false)?];

        let binding = ctx.accounts.extra_account_meta_list.to_account_info();
        let mut data = binding.try_borrow_mut_data()?;
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &account_metas)?;

        msg!("ExtraAccountMetaList initialized for mint: {}", ctx.accounts.mint.key());
        Ok(())
    }

    pub fn transfer_hook(ctx: Context<TransferHook>, _amount: u64) -> Result<()> {
        verify_config(&ctx.accounts.config, &ctx.accounts.mint.key())?;

        let config_data = ctx.accounts.config.try_borrow_data()?;
        let paused = config_data[72] != 0;

        let source_owner = ctx.accounts.source_token_account.owner;
        let destination_owner = ctx.accounts.destination_token_account.owner;

        enforce_blacklist(
            &ctx.accounts.config.key(),
            &source_owner,
            &destination_owner,
            &ctx.accounts.source_blacklist_check.to_account_info(),
            &ctx.accounts.destination_blacklist_check.to_account_info(),
            paused,
        )?;

        msg!("Transfer hook passed for {} tokens", _amount);
        Ok(())
    }

    pub fn fallback(_program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> Result<()> {
        const EXECUTE_DISCRIMINATOR: [u8; 8] = [105, 37, 101, 197, 75, 251, 102, 26];

        if data.len() < 16 {
            return Err(ComplianceError::InvalidInstruction.into());
        }
        if &data[..8] != EXECUTE_DISCRIMINATOR {
            return Err(ComplianceError::InvalidInstruction.into());
        }

        let amount = u64::from_le_bytes(data[8..16].try_into().map_err(|_| ComplianceError::InvalidInstruction)?);

        if accounts.len() < 4 {
            return Err(ComplianceError::InvalidInstruction.into());
        }

        let source_token = &accounts[0];
        let mint = &accounts[1];
        let destination_token = &accounts[2];

        let source_data = source_token.try_borrow_data()?;
        let source_owner = Pubkey::try_from(&source_data[32..64]).map_err(|_| ComplianceError::InvalidInstruction)?;
        drop(source_data);

        let dest_data = destination_token.try_borrow_data()?;
        let destination_owner =
            Pubkey::try_from(&dest_data[32..64]).map_err(|_| ComplianceError::InvalidInstruction)?;
        drop(dest_data);

        let (config_key, _) = Pubkey::find_program_address(&[b"stablecoin", mint.key.as_ref()], &SSS_PROGRAM_ID);

        // Find config by key — works regardless of account position
        let config_account = accounts.iter().find(|a| *a.key == config_key);

        let paused = if let Some(cfg) = config_account {
            let config_data = cfg.try_borrow_data()?;
            config_data.len() > 72 && config_data[72] != 0
        } else {
            false // no config passed — skip pause check
        };

        require!(!paused, ComplianceError::TransfersPaused);

        let (source_blacklist_key, _) =
            Pubkey::find_program_address(&[b"blacklist", config_key.as_ref(), source_owner.as_ref()], &SSS_PROGRAM_ID);
        let (dest_blacklist_key, _) = Pubkey::find_program_address(
            &[b"blacklist", config_key.as_ref(), destination_owner.as_ref()],
            &SSS_PROGRAM_ID,
        );

        let source_blacklisted =
            accounts.iter().find(|a| *a.key == source_blacklist_key).map(|a| a.data_len() > 0).unwrap_or(false);

        let dest_blacklisted =
            accounts.iter().find(|a| *a.key == dest_blacklist_key).map(|a| a.data_len() > 0).unwrap_or(false);

        if source_blacklisted {
            return Err(ComplianceError::SenderBlacklisted.into());
        }
        if dest_blacklisted {
            return Err(ComplianceError::ReceiverBlacklisted.into());
        }

        for (i, a) in accounts.iter().enumerate() {
            msg!("fallback: accounts[{}]={} data_len={}", i, a.key, a.data_len());
        }

        msg!("Fallback: Transfer hook enforced for {} tokens", amount);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = authority,
        space = 8 + ExtraAccountMetaList::size_of(1).unwrap(),
        seeds = [mint.key().as_ref(), crate::ID.as_ref()],
        bump
    )]
    pub extra_account_meta_list: Account<'info, state::ExtraAccountMetaListAccount>,
    /// CHECK: Verified manually in handler — seeds derived from main program ID not hook ID
    pub config: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts, Clone)]
pub struct TransferHook<'info> {
    #[account(token::mint = mint)]
    pub source_token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(token::mint = mint)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,
    /// CHECK: Verified manually in handler — seeds derived from main program ID not hook ID
    pub config: UncheckedAccount<'info>,
    /// CHECK: PDA derived from token account owner — existence = blacklisted
    pub source_blacklist_check: UncheckedAccount<'info>,
    /// CHECK: PDA derived from token account owner — existence = blacklisted
    pub destination_blacklist_check: UncheckedAccount<'info>,
}
