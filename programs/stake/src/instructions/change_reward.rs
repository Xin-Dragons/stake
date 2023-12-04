use anchor_lang::prelude::*;

use crate::{
    state::{Collection, RewardType, Staker},
    utils::{calc_actual_balance, calc_total_emission},
    StakeError,
};

#[derive(Accounts)]
pub struct ChangeReward<'info> {
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
        realloc = collection.current_len() + 16,
        realloc::payer = authority,
        realloc::zero = false
    )]
    pub collection: Account<'info, Collection>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn change_reward_handler(ctx: Context<ChangeReward>, new_reward: u64) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let current_time = Clock::get().unwrap().unix_timestamp;

    let Staker {
        is_active: staking_status,
        ..
    } = **staker;

    let Collection {
        max_stakers_count,
        current_stakers_count,
        staking_ends_at,
        staked_weight,
        current_balance,
        ..
    } = **collection;

    let current_reward = *collection.reward.last().unwrap();
    let last_reward_change_time = *collection.reward_change_time.last().unwrap();

    if Option::is_some(&staking_ends_at) {
        require_gte!(
            staking_ends_at.unwrap(),
            current_time,
            StakeError::StakeOver
        );
    }

    require_eq!(staking_status, true, StakeError::StakeInactive);

    let collection = &mut ctx.accounts.collection;

    match collection.reward_type {
        RewardType::TransferToken => {
            let (current_actual_balance, _accrued_reward, new_staked_weight) = calc_actual_balance(
                current_stakers_count,
                staked_weight,
                current_reward,
                last_reward_change_time,
                staking_ends_at,
                current_time,
                current_balance,
                None,
            )?;

            let new_emission = calc_total_emission(
                new_reward,
                max_stakers_count,
                current_time,
                staking_ends_at.expect("expected end date to be set"),
            )?;

            require_gte!(
                current_actual_balance,
                new_emission,
                StakeError::InsufficientBalanceInVault
            );

            collection.staked_weight = new_staked_weight;
            collection.current_balance = current_actual_balance;
        }
        _ => {}
    }

    collection.change_reward(new_reward, current_time);
    Ok(())
}
