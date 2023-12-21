use anchor_lang::prelude::*;

use crate::{StakeError, STAKING_ENDS, WEIGHT};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct Choice {
    /// reward per second
    pub reward: u64,
    /// time in seconds
    pub duration: i64,
    /// whether to enforce min term
    pub lock: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub enum RewardType {
    Token,
    Selection { options: Vec<Choice> },
    Points,
    Distribution,
}

#[account]
pub struct Emission {
    /// the collection the emission belongs to (32)
    pub collection: Pubkey,
    /// the root hash (1 + 32),
    pub merkle_root: Option<Pubkey>,
    /// The type of emission (1 + 32 + 1)
    pub reward_type: RewardType,
    /// The record of the current and the previous reward emissions (4 + 8)
    pub reward: Vec<u64>,
    /// The record of the time when the emission changed (4 + 8)
    pub reward_change_time: Vec<i64>,
    /// Starting time of the staking (8)
    pub start_time: i64,
    /// optional token mint (1 + 32)
    pub token_mint: Option<Pubkey>,
    /// is token vault (1)
    pub token_vault: bool,
    /// The period for which the staking is funded (1 + 8)
    pub end_time: Option<i64>,
    /// Accrued weight of the staked NFTs (16)
    pub staked_weight: u128,
    /// the current balance for this emission (8)
    pub current_balance: u64,
    /// The minimum stake period to be eligible for reward in seconds (1 + 8)
    pub minimum_period: Option<i64>,
    /// number of staked items using this emission (8)
    pub staked_items: u64,
    /// is the emission active (1)
    pub active: bool,
}

impl Emission {
    pub fn init(
        collection: Pubkey,
        reward_type: RewardType,
        reward: Option<u64>,
        start_time: i64,
        end_time: Option<i64>,
        minimum_period: Option<i64>,
    ) -> Self {
        Self {
            collection,
            reward_type,
            // todo: add this
            merkle_root: None,
            token_mint: None,
            token_vault: false,
            reward: vec![reward.unwrap()],
            reward_change_time: vec![start_time],
            start_time,
            end_time,
            staked_weight: 0,
            current_balance: 0,
            minimum_period,
            staked_items: 0,
            active: true,
        }
    }

    pub fn current_len(&self) -> usize {
        std::mem::size_of::<Emission>() + self.reward.len() * 16
    }

    pub fn change_reward(&mut self, new_reward: u64, current_time: i64) {
        self.reward.push(new_reward);
        self.reward_change_time.push(current_time);
    }

    pub fn increase_staked_items(&mut self) -> Result<()> {
        self.staked_items = self
            .staked_items
            .checked_add(1)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }

    pub fn decrease_staked_items(&mut self) -> Result<()> {
        self.staked_items = self
            .staked_items
            .checked_sub(1)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }

    pub fn extend_staking(&mut self, new_end_time: i64) {
        self.end_time = Some(new_end_time);
    }

    pub fn update_staked_weight(&mut self, stake_time: i64, increase_weight: bool) -> Result<()> {
        let weight = self.get_staked_weight(stake_time)?;

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

    pub fn get_staked_weight(&mut self, stake_time: i64) -> Result<(u128)> {
        let last_reward_time = *self.reward_change_time.last().unwrap();

        let base = self
            .end_time
            .unwrap_or(STAKING_ENDS)
            .checked_sub(last_reward_time)
            .ok_or(StakeError::ProgramSubError)? as u128; // directly converting to u128 since it can't be negative

        let weight_time = stake_time.max(last_reward_time);

        let mut num = self
            .end_time
            .unwrap_or(STAKING_ENDS)
            .checked_sub(weight_time)
            .ok_or(StakeError::ProgramSubError)? as u128; // directly converting to u128 since it can't be negative

        num = num.checked_mul(WEIGHT).ok_or(StakeError::ProgramMulError)?;

        msg!("NUM {} BASE {}", num, base);

        let weight = num.checked_div(base).ok_or(StakeError::ProgramDivError)?;

        Ok(weight)
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
        let cutoff_time = current_time.min(self.end_time.unwrap_or(STAKING_ENDS));

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

        msg!("REMOVING {}, {}", reward_since_change, self.current_balance);

        self.current_balance = self
            .current_balance
            .checked_sub(reward_since_change)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }
}
