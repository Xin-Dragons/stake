use anchor_lang::prelude::*;
use anchor_spl::token::{
    mint_to, set_authority, spl_token::instruction::AuthorityType, transfer, Mint, MintTo,
    SetAuthority, Token, TokenAccount, Transfer,
};

use crate::{
    state::{Collection, Emission, RewardType, Staker},
    utils::calc_actual_balance,
    StakeError, STAKING_ENDS,
};

#[derive(Accounts)]
pub struct CloseEmission<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection.collection_mint.as_ref(),
            b"collection", 
        ],
        bump = collection.bump,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        has_one = collection
    )]
    pub emission: Account<'info, Emission>,

    #[account(
        mut,
        address = staker.token_mint.unwrap() @ StakeError::InvalidRewardToken
    )]
    pub token_mint: Option<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Option<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_authority,
    )]
    pub stake_token_vault: Option<Account<'info, TokenAccount>>,

    /// CHECK: This account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority",
        ],
        bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> CloseEmission<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .stake_token_vault
                .as_ref()
                .expect("stake_token_vault expected")
                .to_account_info(),
            to: self
                .token_account
                .as_ref()
                .expect("token_account expected")
                .to_account_info(),
            authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn close_emission_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, CloseEmission<'info>>,
) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;
    let current_time = Clock::get().unwrap().unix_timestamp;

    let Staker {
        token_auth_bump, ..
    } = **staker;

    let Emission {
        end_time,
        staked_weight,
        staked_items,
        ..
    } = **emission;

    let reward_type = &emission.reward_type;
    let current_reward = *emission.reward.last().unwrap();
    let last_reward_change_time = *emission.reward_change_time.last().unwrap();
    let staker_key = staker.key();

    let token_auth_seed = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"token-authority"[..],
        &[token_auth_bump],
    ];

    let mut tokens_to_reclaim: u64 = 0;

    match reward_type {
        RewardType::Selection { options } => {
            // require_eq!(emission.staked_items, 0, StakeError::CollectionHasStakers);
        }
        RewardType::Token => {
            if staker.token_vault {
                // require_eq!(emission.staked_items, 0, StakeError::CollectionHasStakers);

                let (current_actual_balance, _accrued_reward, _new_staked_weight) =
                    calc_actual_balance(
                        staked_items,
                        staked_weight,
                        current_reward,
                        last_reward_change_time,
                        end_time,
                        current_time,
                        emission.current_balance,
                        None,
                    )?;
                if current_actual_balance > 0 {
                    transfer(
                        ctx.accounts
                            .transfer_token_ctx()
                            .with_signer(&[&token_auth_seed[..]]),
                        current_actual_balance,
                    )?;
                    tokens_to_reclaim = current_actual_balance;
                }
            }
        }
        _ => {}
    }

    let collection: &mut Account<'_, Collection> = &mut ctx.accounts.collection;

    match reward_type {
        RewardType::Selection { options: _ } => {
            collection.selection_emission = None;
        }
        RewardType::Distribution => {
            collection.distribution_emission = None;
        }
        RewardType::Points => {
            collection.points_emission = None;
        }
        RewardType::Token => {
            collection.token_emission = None;

            if staker.token_vault && tokens_to_reclaim > 0 {
                let emission = &mut ctx.accounts.emission;
                emission.current_balance = emission
                    .current_balance
                    .checked_sub(tokens_to_reclaim)
                    .ok_or(StakeError::ProgramSubError)?;
            }
            // if !token_vault && tokens_to_transfer > 0 {
            //     let emission = &mut ctx.accounts.emission;
            //     // update to token_vault so future owing tokens are transferred not minted.
            //     collection.token_vault = true;
            //     emission.increase_current_balance(tokens_to_transfer)?;
            // }
        }
    }

    let emission = &mut ctx.accounts.emission;

    emission.active = false;

    // Allow stakers to instantly withdraw their NFTs
    emission.minimum_period = None;

    // If the staking end time is more than the current time then change it to current
    // This is done to avoid accrual of any new stake rewards
    emission.end_time = if end_time.unwrap_or(STAKING_ENDS) > current_time {
        Some(current_time)
    } else {
        end_time
    };

    Ok(())
}
