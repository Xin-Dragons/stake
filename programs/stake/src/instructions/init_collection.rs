use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{Metadata, MetadataAccount},
    token::{
        set_authority, spl_token::instruction::AuthorityType, transfer, Mint, SetAuthority, Token,
        TokenAccount, Transfer,
    },
};

use crate::{
    constants::{SUBSCRIPTION_WALLET, USDC_MINT_PUBKEY},
    state::{Collection, ProgramConfig, RewardType, Staker},
    utils::{calc_pro_rata_fee, calc_total_emission},
    StakeError,
};

#[derive(Accounts)]
pub struct InitCollection<'info> {
    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        realloc = staker.current_len() + 32,
        realloc::payer = authority,
        realloc::zero = false,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Box<Account<'info, Staker>>,

    #[account(
        init,
        payer = authority,
        space = Collection::LEN,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection_mint.key().as_ref(),
            b"collection",
        ],
        bump
    )]
    pub collection: Box<Account<'info, Collection>>,

    #[account(
        seeds = [
            b"metadata",
            Metadata::id().as_ref(),
            collection_mint.key().as_ref()
        ],
        seeds::program = Metadata::id(),
        bump,
    )]
    collection_metadata: Option<Box<Account<'info, MetadataAccount>>>,

    #[account(mint::decimals = 0)]
    pub collection_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = token_authority
    )]
    pub stake_token_vault: Option<Box<Account<'info, TokenAccount>>>,

    /// CHECK: This account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub token_mint: Option<Box<Account<'info, Mint>>>,

    #[account(
        mut,
        associated_token::mint = usdc,
        associated_token::authority = authority
    )]
    pub usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = usdc,
        associated_token::authority = subscription_wallet
    )]
    pub subscription_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(address = SUBSCRIPTION_WALLET)]
    pub subscription_wallet: SystemAccount<'info>,

    #[account(address = USDC_MINT_PUBKEY)]
    pub usdc: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Option<Box<Account<'info, TokenAccount>>>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> InitCollection<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .token_account
                .as_ref()
                .expect("token_account is expected")
                .to_account_info(),
            to: self
                .stake_token_vault
                .as_ref()
                .expect("stake_token_vault is expected")
                .to_account_info(),
            authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn transfer_subscription_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.usdc_account.to_account_info(),
            to: self.subscription_usdc_account.to_account_info(),
            authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn transfer_auth_ctx(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self
                .token_mint
                .as_ref()
                .expect("token_mint expected")
                .to_account_info(),
            current_authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn init_collection_handler(
    ctx: Context<InitCollection>,
    custodial: bool,
    reward_type: RewardType,
    reward: u64,
    minimum_period: i64,
    staking_starts_at: Option<i64>,
    duration: Option<i64>,
    max_stakers_count: u64,
    lock_for_minimum_period: bool,
) -> Result<()> {
    let clock = Clock::get().unwrap();
    let current_time = clock.unix_timestamp;
    let staker = &ctx.accounts.staker;

    let start_time = staking_starts_at.unwrap_or(current_time);

    require_gte!(start_time, current_time, StakeError::StartTimeInPast);

    if lock_for_minimum_period {
        require_gte!(
            60 * 60 * 24 * 365,
            minimum_period,
            StakeError::LockingPeriodTooLong
        );
        require_gt!(minimum_period, 0, StakeError::LockingPeriodTooShort);
        require_keys_eq!(
            ctx.accounts
                .collection_metadata
                .as_ref()
                .expect("collection_metadata required for min-period lock")
                .update_authority,
            ctx.accounts.authority.key(),
            StakeError::UpdateAuthRequired
        )
    }

    match reward_type {
        RewardType::TransferToken => {
            require!(Option::is_some(&duration), StakeError::DurationRequired);
            require_gt!(duration.unwrap(), 0, StakeError::DurationTooShort);
            require_gte!(minimum_period, 0, StakeError::NegativePeriodValue);
        }
        _ => {}
    }

    let Staker {
        next_payment_time, ..
    } = **staker.as_ref();

    let actual_end_date = if Option::is_some(&duration) {
        Some(start_time + duration.unwrap())
    } else {
        None
    };

    if staker.collections.len() > 0 {
        let to_pay = calc_pro_rata_fee(
            next_payment_time,
            ctx.accounts.program_config.extra_collection_fee,
        )?;

        if to_pay > 0 {
            transfer(ctx.accounts.transfer_subscription_ctx(), to_pay)?;
        }
    }

    let collection = &mut ctx.accounts.collection;

    ***collection = Collection::init(
        ctx.accounts.staker.key(),
        ctx.accounts.collection_mint.key(),
        custodial,
        reward_type,
        ctx.accounts.token_mint.as_ref().map(|t| t.key()),
        reward,
        current_time,
        max_stakers_count,
        start_time,
        actual_end_date,
        minimum_period,
        lock_for_minimum_period,
        ctx.bumps.collection,
    );

    match reward_type {
        RewardType::MintToken => {
            let mint_auth = ctx
                .accounts
                .token_mint
                .as_ref()
                .map(|o| o.mint_authority)
                .expect("token_mint expecte")
                .unwrap();
            let token_auth = ctx.accounts.token_authority.key();
            if !token_auth.eq(&mint_auth) {
                set_authority(
                    ctx.accounts.transfer_auth_ctx(),
                    AuthorityType::MintTokens,
                    Some(ctx.accounts.token_authority.key()),
                )?;
            }
        }
        RewardType::TransferToken => {
            let total_emission = calc_total_emission(
                reward,
                max_stakers_count,
                start_time,
                actual_end_date.unwrap(),
            )?;

            transfer(ctx.accounts.transfer_token_ctx(), total_emission)?;
            let collection = &mut ctx.accounts.collection;

            collection.increase_current_balance(total_emission)?;
        }
        _ => (),
    }

    let staker: &mut Account<'_, Staker> = &mut ctx.accounts.staker;
    staker.add_collection(ctx.accounts.collection.key())
}
