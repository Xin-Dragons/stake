use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{Metadata, MetadataAccount},
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};

use crate::{
    state::{Collection, Emission, RewardType, Staker},
    utils::calc_total_emission,
    StakeError,
};

#[derive(Accounts)]
pub struct AddEmission<'info> {
    #[account(
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection.collection_mint.as_ref(),
            b"collection"
        ],
        bump = collection.bump
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        seeds = [
            b"metadata",
            Metadata::id().as_ref(),
            collection_mint.as_ref().unwrap().key().as_ref()
        ],
        seeds::program = Metadata::id(),
        bump,
    )]
    collection_metadata: Option<Box<Account<'info, MetadataAccount>>>,

    #[account(mint::decimals = 0)]
    pub collection_mint: Option<Box<Account<'info, Mint>>>,

    #[account(
        init,
        payer = authority,
        space = std::mem::size_of::<Emission>()
    )]
    pub emission: Account<'info, Emission>,

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

    #[account()]
    pub token_mint: Option<Box<Account<'info, Mint>>>,

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

impl<'info> AddEmission<'info> {
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
}

pub fn add_emission_handler(
    ctx: Context<AddEmission>,
    reward_type: RewardType,
    reward: Option<u64>,
    start_time: Option<i64>,
    duration: Option<i64>,
    minimum_period: Option<i64>,
    starting_balance: Option<u64>,
) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let current_time = Clock::get().unwrap().unix_timestamp;
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;
    let collection_key = collection.key();

    let start_time = start_time.unwrap_or(current_time);

    let end_time = if Option::is_some(&duration) {
        Some(start_time + duration.unwrap())
    } else {
        None
    };

    let Collection {
        max_stakers_count, ..
    } = *ctx.accounts.collection;

    match reward_type.clone() {
        RewardType::Selection { options } => {
            if staker.token_vault {
                require!(Option::is_some(&duration), StakeError::DurationRequired);
                require_gt!(duration.unwrap(), 0, StakeError::DurationTooShort);
                require_gte!(
                    minimum_period.unwrap_or(0),
                    0,
                    StakeError::NegativePeriodValue
                );
                require!(Option::is_some(&end_time), StakeError::StakeEndTimeRequired);
            }
            require!(
                Option::is_none(&collection.selection_emission),
                StakeError::SelectionEmissionExists
            );
            require!(
                Option::is_none(&minimum_period),
                StakeError::NoMinPeriodWithOption
            );
            require!(
                Option::is_some(&ctx.accounts.token_mint),
                StakeError::TokenMintRequired
            );

            let is_locking = options.iter().any(|opt| opt.lock);

            if is_locking {
                require_gte!(
                    60 * 60 * 24 * 365,
                    minimum_period.unwrap(),
                    StakeError::LockingPeriodTooLong
                );
                require_gt!(
                    minimum_period.unwrap(),
                    0,
                    StakeError::LockingPeriodTooShort
                );
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

            require!(starting_balance > Some(0), StakeError::InvalidEmission);

            let collection = &mut ctx.accounts.collection;
            collection.selection_emission = Some(emission.key());
        }
        RewardType::Token => {
            if staker.token_vault {
                require!(Option::is_some(&duration), StakeError::DurationRequired);
                require_gt!(duration.unwrap(), 0, StakeError::DurationTooShort);
                require_gte!(
                    minimum_period.unwrap_or(0),
                    0,
                    StakeError::NegativePeriodValue
                );
                require!(Option::is_some(&end_time), StakeError::StakeEndTimeRequired);
            }
            require!(
                Option::is_none(&collection.token_emission),
                StakeError::SelectionEmissionExists
            );
            require!(Option::is_some(&reward), StakeError::RewardRequired);
            require!(
                Option::is_some(&ctx.accounts.token_mint),
                StakeError::TokenMintRequired
            );
            let collection = &mut ctx.accounts.collection;
            collection.token_emission = Some(emission.key());
        }
        RewardType::Distribution => {
            require!(
                Option::is_none(&collection.distribution_emission),
                StakeError::SelectionEmissionExists
            );
            let collection = &mut ctx.accounts.collection;
            collection.distribution_emission = Some(emission.key());
        }
        RewardType::Points => {
            require!(
                Option::is_none(&collection.points_emission),
                StakeError::SelectionEmissionExists
            );
            let collection = &mut ctx.accounts.collection;
            collection.points_emission = Some(emission.key());
        }
        _ => {}
    }

    let is_token = match reward_type.clone() {
        RewardType::Token => true,
        RewardType::Selection { options: _ } => true,
        _ => false,
    };

    let balance_increase: u64 = match reward_type {
        RewardType::Token => {
            if staker.token_vault {
                let total_emission = calc_total_emission(
                    reward.unwrap(),
                    max_stakers_count,
                    start_time,
                    end_time.unwrap(),
                )?;

                transfer(ctx.accounts.transfer_token_ctx(), total_emission)?;

                total_emission
            } else {
                0
            }
        }
        RewardType::Selection { options: _ } => {
            if staker.token_vault {
                let amount = starting_balance.unwrap();
                transfer(ctx.accounts.transfer_token_ctx(), amount)?;
                amount
            } else {
                0
            }
        }
        _ => 0,
    };

    let emission = &mut ctx.accounts.emission;

    **emission = Emission::init(
        collection_key,
        reward_type.clone(),
        reward,
        start_time,
        end_time,
        minimum_period,
    );

    if is_token && staker.token_vault {
        emission.increase_current_balance(balance_increase)?;
    }

    Ok(())
}
