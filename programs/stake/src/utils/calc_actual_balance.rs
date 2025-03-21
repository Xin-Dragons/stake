use anchor_lang::prelude::*;

use crate::{StakeError, STAKING_ENDS, WEIGHT};

pub fn calc_actual_balance(
    current_stakers_count: u64,
    staked_weight: u128,
    last_reward_rate: u64,
    last_reward_time: i64,
    staking_ends_at: Option<i64>,
    current_time: i64,
    current_balance: u64,
    new_end_time: Option<i64>,
) -> Result<(u64, u64, u128)> {
    let staking_ends_at = staking_ends_at.unwrap_or(STAKING_ENDS);
    // if no current stakers, return the full balance
    if current_stakers_count == 0 {
        return Ok((current_balance, 0, 0));
    }
    let avg_staked_weight = if staked_weight == 0 {
        staked_weight
    } else {
        staked_weight
            .checked_div(current_stakers_count as u128)
            .ok_or(StakeError::ProgramDivError)?
            + 1
    };

    msg!("FIRST SUB");

    // total time since last reward change to stake end
    let total_time = staking_ends_at
        .checked_sub(last_reward_time)
        .ok_or(StakeError::ProgramSubError)?;

    let total_time_u128 = match u128::try_from(total_time) {
        Ok(time) => time,
        _ => {
            return err!(StakeError::FailedTimeConversion);
        }
    };

    // time between average staking time and stake end;
    let stake_to_end_time_weighted = total_time_u128
        .checked_mul(avg_staked_weight)
        .ok_or(StakeError::ProgramMulError)?;

    let stake_to_end_time = stake_to_end_time_weighted
        .checked_div(WEIGHT)
        .ok_or(StakeError::ProgramDivError)?
        + 1;

    let stake_to_end_time = match u64::try_from(stake_to_end_time) {
        Ok(time) => time,
        _ => {
            return err!(StakeError::FailedTimeConversion);
        }
    };
    msg!("SECOND SUB");

    // calculate rewardable time
    let rewardable_time = if staking_ends_at > current_time {
        // if the current time is less than the stake end time
        // subtract the unaccrued time from the stake to end time
        let unaccrued_time = staking_ends_at
            .checked_sub(current_time)
            .ok_or(StakeError::ProgramSubError)?;

        let unaccrued_time_u64 = match u64::try_from(unaccrued_time) {
            Ok(time) => time,
            _ => {
                return err!(StakeError::FailedTimeConversion);
            }
        };

        msg!("THIRD SUB");

        stake_to_end_time
            .checked_sub(unaccrued_time_u64)
            .ok_or(StakeError::ProgramSubError)?
    } else {
        msg!("FOURTH SUB");
        // if the current time is greater or equal to the stake end time
        // add seconds since the stake end time to the rewardable time
        let accrued_time = current_time
            .checked_sub(staking_ends_at)
            .ok_or(StakeError::ProgramSubError)?;

        msg!("FOURTH SUB SUCCESS");

        let accrued_time_u64 = match u64::try_from(accrued_time) {
            Ok(time) => time,
            _ => {
                return err!(StakeError::FailedTimeConversion);
            }
        };

        stake_to_end_time
            .checked_add(accrued_time_u64)
            .ok_or(StakeError::ProgramAddError)?
    };

    // the rewards yet to be paid (per staker)
    let accrued_reward = last_reward_rate
        .checked_mul(rewardable_time)
        .ok_or(StakeError::ProgramMulError)?;

    // the rewards yet to be paid (all stakers)
    let accrued_reward = accrued_reward
        .checked_mul(current_stakers_count)
        .ok_or(StakeError::ProgramMulError)?;

    // the calculation of the new staked weight
    let new_staked_weight = match new_end_time {
        Some(new_time) => {
            msg!("STAKE TO OLD END {}", stake_to_end_time);
            let stake_to_old_end = match i64::try_from(stake_to_end_time) {
                Ok(time) => time,
                _ => {
                    return err!(StakeError::FailedTimeConversion);
                }
            };

            let time_added = new_time
                .checked_sub(staking_ends_at)
                .ok_or(StakeError::ProgramSubError)?;

            // add extended time to stake period
            let stake_to_new_end = stake_to_old_end
                .checked_add(time_added)
                .ok_or(StakeError::ProgramAddError)?;

            let new_base = new_time
                .checked_sub(last_reward_time)
                .ok_or(StakeError::ProgramSubError)?;

            let stake_to_new_end_u128 = match u128::try_from(stake_to_new_end) {
                Ok(time) => time,
                _ => {
                    return err!(StakeError::FailedTimeConversion);
                }
            };

            let new_base_u128 = match u128::try_from(new_base) {
                Ok(time) => time,
                _ => {
                    return err!(StakeError::FailedTimeConversion);
                }
            };

            let new_num = stake_to_new_end_u128
                .checked_mul(WEIGHT)
                .ok_or(StakeError::ProgramMulError)?;

            // new average staked weight
            let new_weight = new_num
                .checked_div(new_base_u128)
                .ok_or(StakeError::ProgramDivError)?;

            new_weight
                .checked_mul(current_stakers_count as u128)
                .ok_or(StakeError::ProgramMulError)?
        }
        None => WEIGHT
            .checked_mul(current_stakers_count as u128)
            .ok_or(StakeError::ProgramMulError)?,
    };

    if current_balance == 0 {
        return Ok((0, accrued_reward, new_staked_weight));
    }

    msg!("LAST ONE, {}, {}", current_balance, accrued_reward);

    let current_actual_balance = current_balance
        .checked_sub(accrued_reward)
        .ok_or(StakeError::ProgramSubError)?;

    Ok((current_actual_balance, accrued_reward, new_staked_weight))
}
