use anchor_lang::prelude::*;

use crate::{
    state::{Collection, Emission, RewardType, Staker},
    utils::{calc_actual_balance, calc_total_emission},
    StakeError,
};

#[derive(Accounts)]
pub struct ExtendEmission<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(
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

    pub authority: Signer<'info>,
}

pub fn extend_emission_handler(ctx: Context<ExtendEmission>, new_ending_time: i64) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;
    let current_time = Clock::get().unwrap().unix_timestamp;

    let Staker {
        is_active: staking_status,
        ..
    } = **staker;

    let Collection {
        max_stakers_count,
        current_stakers_count,
        ..
    } = **collection;

    let Emission {
        end_time,
        staked_weight,
        current_balance,
        ..
    } = **emission;

    let current_reward = *emission.reward.last().unwrap();
    let last_reward_change_time = *emission.reward_change_time.last().unwrap();

    match emission.reward_type {
        RewardType::Token => {}
        _ => return err!(StakeError::InvalidEmission),
    }

    require!(
        Option::is_some(&end_time),
        StakeError::CannotExtendNoEndDate
    );
    require_eq!(staking_status, true, StakeError::StakeInactive);
    require_gt!(
        new_ending_time,
        current_time,
        StakeError::InvalidStakeEndTime
    );
    require_gt!(
        new_ending_time,
        end_time.unwrap(),
        StakeError::InvalidStakeEndTime
    );

    let (current_actual_balance, _accrued_reward, new_staked_weight) = calc_actual_balance(
        current_stakers_count,
        staked_weight,
        current_reward,
        last_reward_change_time,
        end_time,
        current_time,
        current_balance,
        Some(new_ending_time),
    )?;

    let new_emission = calc_total_emission(
        current_reward,
        max_stakers_count,
        current_time,
        new_ending_time,
    )?;

    require_gte!(
        current_actual_balance,
        new_emission,
        StakeError::InsufficientBalanceInVault
    );

    let emission = &mut ctx.accounts.emission;

    emission.extend_staking(new_ending_time);
    emission.staked_weight = new_staked_weight;

    Ok(())
}
