use anchor_lang::prelude::{
    borsh::{BorshDeserialize, BorshSerialize},
    *,
};

use crate::{StakeError, STAKING_ENDS, WEIGHT};

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug)]
pub enum RewardType {
    TransferToken,
    MintToken,
    Points,
    SolDistribution,
    TokenDistribution,
    None,
}

#[account]
pub struct Collection {
    /// staker this collection belongs to (32)
    pub staker: Pubkey,
    /// MCC mint of the collection (32)
    pub collection_mint: Pubkey,
    /// Collection custody type (1)
    pub custodial: bool,
    /// Type of reward (1)
    pub reward_type: RewardType,
    /// Reward token for the collection (1 + 32)
    pub reward_token: Option<Pubkey>,
    /// items can be staked, rewards accrue (1)
    pub is_active: bool,
    /// The record of the current and the previous reward emissions (12)
    pub reward: Vec<u64>,
    /// The record of the time when the emission changed (12)
    pub reward_change_time: Vec<i64>,
    /// The max number of NFTs that can be staked (8)
    pub max_stakers_count: u64,
    /// The current number of NFTs staked (8)
    pub current_stakers_count: u64,
    /// Accrued weight of the staked NFTs (16)
    pub staked_weight: u128,
    /// Starting time of the staking (8)
    pub staking_starts_at: i64,
    /// The period for which the staking is funded (1 + 8)
    pub staking_ends_at: Option<i64>,
    /// The minimum stake period to be eligible for a reward in seconds (8)
    pub minimum_period: i64,
    /// Disallow unstaking in minimum period (1)
    /// REQUIRES UPDATE AUTH APPROVAL
    pub lock_for_minimum_period: bool,
    /// Bump of the Collection PDA (1)
    pub bump: u8,
    /// the current balance for this collection (8)
    pub current_balance: u64,
}

impl Collection {
    pub const LEN: usize =
        8 + 32 + 32 + 1 + 1 + (1 + 32) + 1 + 12 + 12 + 8 + 8 + 16 + 8 + (1 + 8) + 8 + 1 + 1 + 8;

    pub fn init(
        staker: Pubkey,
        collection_mint: Pubkey,
        custodial: bool,
        reward_type: RewardType,
        reward_token: Option<Pubkey>,
        reward: u64,
        start_time: i64,
        max_stakers_count: u64,
        staking_starts_at: i64,
        staking_ends_at: Option<i64>,
        minimum_period: i64,
        lock_for_minimum_period: bool,
        bump: u8,
    ) -> Self {
        Self {
            staker,
            collection_mint,
            custodial,
            reward_type,
            reward_token,
            is_active: true,
            reward: vec![reward],
            reward_change_time: vec![start_time],
            max_stakers_count,
            staking_starts_at,
            staking_ends_at,
            minimum_period,
            staked_weight: 0,
            current_stakers_count: 0,
            lock_for_minimum_period,
            bump,
            current_balance: 0,
        }
    }

    pub fn extend_staking(&mut self, new_end_time: i64) {
        self.staking_ends_at = Some(new_end_time);
    }

    pub fn change_reward(&mut self, new_reward: u64, current_time: i64) {
        self.reward.push(new_reward);
        self.reward_change_time.push(current_time);
    }

    pub fn current_len(&self) -> usize {
        Collection::LEN - 16 + self.reward.len() * 16
    }

    pub fn increase_staker_count(&mut self) -> Result<()> {
        self.current_stakers_count = self
            .current_stakers_count
            .checked_add(1)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }

    pub fn decrease_staker_count(&mut self) -> Result<()> {
        self.current_stakers_count = self
            .current_stakers_count
            .checked_sub(1)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }

    pub fn update_staked_weight(&mut self, stake_time: i64, increase_weight: bool) -> Result<()> {
        let last_reward_time = *self.reward_change_time.last().unwrap();

        let base = self
            .staking_ends_at
            .unwrap_or(STAKING_ENDS)
            .checked_sub(last_reward_time)
            .ok_or(StakeError::ProgramSubError)? as u128; // directly converting to u128 since it can't be negative

        let weight_time = stake_time.max(last_reward_time);

        let mut num = self
            .staking_ends_at
            .unwrap_or(STAKING_ENDS)
            .checked_sub(weight_time)
            .ok_or(StakeError::ProgramSubError)? as u128; // directly converting to u128 since it can't be negative

        num = num.checked_mul(WEIGHT).ok_or(StakeError::ProgramMulError)?;

        msg!("NUM {} BASE {}", num, base);

        let weight = num.checked_div(base).ok_or(StakeError::ProgramDivError)?;

        msg!("ADDING WEIGHT, {}, {}", self.staked_weight, weight);

        if increase_weight {
            self.staked_weight = self
                .staked_weight
                .checked_add(weight)
                .ok_or(StakeError::ProgramAddError)?;
        } else {
            self.staked_weight = self
                .staked_weight
                .checked_sub(weight)
                .ok_or(StakeError::ProgramSubError)?;
        }

        Ok(())
    }

    pub fn increase_current_balance(&mut self, added_funds: u64) -> Result<()> {
        msg!("ADDING {}", added_funds);
        self.current_balance = self
            .current_balance
            .checked_add(added_funds)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }

    pub fn decrease_current_balance(&mut self, staked_at: i64, current_time: i64) -> Result<()> {
        let last_reward_time = *self.reward_change_time.last().unwrap();
        let last_reward = *self.reward.last().unwrap();

        let reward_time = staked_at.max(last_reward_time);
        let cutoff_time = current_time.min(self.staking_ends_at.unwrap_or(STAKING_ENDS));

        let rewardable_time_since_change = cutoff_time
            .checked_sub(reward_time)
            .ok_or(StakeError::ProgramSubError)?;

        let rewardable_time_u64 = match u64::try_from(rewardable_time_since_change) {
            Ok(time) => time,
            _ => {
                return err!(StakeError::FailedTimeConversion);
            }
        };

        let reward_since_change = last_reward
            .checked_mul(rewardable_time_u64)
            .ok_or(StakeError::ProgramMulError)?;

        msg!("REMOVING {}", reward_since_change);

        self.current_balance = self
            .current_balance
            .checked_sub(reward_since_change)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }

    pub fn close_collection(&mut self) {
        self.is_active = false;
    }
}
