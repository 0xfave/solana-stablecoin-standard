use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount};
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

#[program]
pub mod sss_compliance_hook {
    use super::*;

    pub fn initialize_extra_account_meta_list(ctx: Context<InitializeExtraAccountMetaList>) -> Result<()> {
        msg!("ExtraAccountMetaList initialized for mint: {}", ctx.accounts.mint.key());
        Ok(())
    }

    pub fn transfer_hook(ctx: Context<TransferHook>, amount: u64) -> Result<()> {
        let config = &ctx.accounts.config;

        require!(!config.paused, ComplianceError::TransfersPaused);

        let source_owner = ctx.accounts.source_owner.key();
        let destination_owner = ctx.accounts.destination_owner.key();
        let config_key = config.key();

        let blacklist_seed = b"blacklist";
        let program_id = crate::ID;

        let source_blacklist_key = Pubkey::try_find_program_address(
            &[blacklist_seed, config_key.as_ref(), source_owner.as_ref()],
            &program_id,
        )
        .map(|(key, _)| key);

        if let Some(blacklist_key) = source_blacklist_key {
            if *ctx.accounts.source_blacklist_check.key == blacklist_key
                && ctx.accounts.source_blacklist_check.data_len() > 0
            {
                return Err(ComplianceError::SenderBlacklisted.into());
            }
        }

        let dest_blacklist_key = Pubkey::try_find_program_address(
            &[blacklist_seed, config_key.as_ref(), destination_owner.as_ref()],
            &program_id,
        )
        .map(|(key, _)| key);

        if let Some(blacklist_key) = dest_blacklist_key {
            if *ctx.accounts.destination_blacklist_check.key == blacklist_key
                && ctx.accounts.destination_blacklist_check.data_len() > 0
            {
                return Err(ComplianceError::ReceiverBlacklisted.into());
            }
        }

        msg!("Transfer hook passed for {} tokens from {} to {}", amount, source_owner, destination_owner);

        Ok(())
    }

    pub fn fallback(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> Result<()> {
        let instruction = TransferHookInstruction::unpack(data)?;

        match instruction {
            TransferHookInstruction::Execute { amount } => {
                msg!("Fallback: Executing transfer hook with amount: {}", amount);
                Ok(())
            }
            _ => Err(ComplianceError::InvalidInstruction.into()),
        }
    }
}

#[derive(Accounts)]
pub struct InitializeExtraAccountMetaList<'info> {
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = authority,
        space = state::ExtraAccountMetaListAccount::INIT_SPACE,
        seeds = [mint.key().as_ref(), crate::ID.as_ref()],
        bump
    )]
    pub extra_account_meta_list: Account<'info, state::ExtraAccountMetaListAccount>,
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts, Clone)]
pub struct TransferHook<'info> {
    #[account(token::mint = mint, token::authority = source_owner)]
    pub source_token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(token::mint = mint, token::authority = destination_owner)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,
    pub source_owner: Signer<'info>,
    pub destination_owner: Signer<'info>,
    #[account(
        seeds = [b"stablecoin", mint.key().as_ref()],
        bump = config.bump
    )]
    pub config: Account<'info, StablecoinConfig>,
    /// CHECK: This is a PDA derived from blacklist seeds - account existence is checked at runtime
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), source_owner.key().as_ref()],
        bump
    )]
    pub source_blacklist_check: UncheckedAccount<'info>,
    /// CHECK: This is a PDA derived from blacklist seeds - account existence is checked at runtime
    #[account(
        seeds = [b"blacklist", config.key().as_ref(), destination_owner.key().as_ref()],
        bump
    )]
    pub destination_blacklist_check: UncheckedAccount<'info>,
}

#[account]
#[derive(InitSpace)]
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
#[derive(InitSpace)]
pub struct BlacklistEntry {
    pub blacklister: Pubkey,
    #[max_len(200)]
    pub reason: String,
    pub timestamp: i64,
    pub bump: u8,
}
