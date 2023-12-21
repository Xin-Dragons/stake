use anchor_lang::prelude::*;

use crate::StakeError;

use super::{ProgramConfig, Theme};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum Subscription {
    Penalty,
    Free,
    Advanced,
    Pro,
    Ultimate,
    Custom {
        amount: u64,
        stake_fee: u64,
        unstake_fee: u64,
        claim_fee: u64,
    },
}

#[account]
pub struct Staker {
    /// The authority of the staker (32)
    pub authority: Pubkey,
    /// slug, max 50 chars (50 + 4)
    pub slug: String,
    /// name of the project, max 50 chars (50 + 4)
    pub name: String,
    /// optional custom domain, max 50 chars (1 + 4 + 50),
    pub custom_domain: Option<String>,
    /// Active theme struct
    pub theme: Theme,
    /// Staker status (1)
    pub is_active: bool,
    /// Branding removed (1)
    pub remove_branding: bool,
    /// Branding removed (1)
    pub own_domain: bool,
    /// Subscription level (1 + 32)
    pub subscription: Subscription,
    /// Subscription level (1 + 32)
    pub prev_subscription: Subscription,
    /// Date the subscription will become live (8)
    pub subscription_live_date: i64,
    // list of associated collections (init empty) 4
    pub collections: Vec<Pubkey>,
    /// The bump of the token authority PDA (1)
    pub token_auth_bump: u8,
    /// The bump of the NFT authority PDA (1)
    pub nft_auth_bump: u8,
    /// staking start time  (8)
    pub start_date: i64,
    /// optional token mint with mint auth (1 + 32)
    pub token_mint: Option<Pubkey>,
    /// use a token vault (1)
    pub token_vault: bool,
    /// timestamp the next payment is due  (8)
    pub next_payment_time: i64,
    /// number of staked items (4)
    pub number_staked: u32,
}

impl Staker {
    pub const LEN: usize = 8
        + 32
        + (4 + 50)
        + (4 + 50)
        + (1 + 4 + 50)
        + std::mem::size_of::<Theme>()
        + 1
        + 1
        + 1
        + (1 + 32)
        + (1 + 32)
        + 8
        + 4
        + 1
        + 1
        + 8
        + (1 + 32)
        + 1
        + 8
        + 4;

    pub fn init(
        slug: String,
        name: String,
        authority: Pubkey,
        remove_branding: bool,
        own_domain: bool,
        subscription: Subscription,
        token_auth_bump: u8,
        nft_auth_bump: u8,
        start_date: i64,
        next_payment_time: i64,
    ) -> Self {
        Self {
            slug: slug.to_owned(),
            name: name.to_owned(),
            theme: Theme::default(),
            custom_domain: None,
            is_active: false,
            token_vault: false,
            remove_branding,
            own_domain,
            subscription,
            prev_subscription: subscription,
            subscription_live_date: start_date,
            authority,
            token_mint: None,
            token_auth_bump,
            nft_auth_bump,
            collections: vec![],
            start_date,
            next_payment_time,
            number_staked: 0,
        }
    }

    pub fn current_len(&self) -> usize {
        Staker::LEN + self.collections.len() * 32
    }

    pub fn add_collection(&mut self, collection: Pubkey) -> Result<()> {
        self.collections.push(collection);
        Ok(())
    }

    pub fn close_staker(&mut self) {
        self.is_active = false;
    }

    pub fn set_own_domain(&mut self, own_domain: bool) {
        self.own_domain = own_domain;
    }

    pub fn set_remove_branding(&mut self, remove_branding: bool) {
        self.remove_branding = remove_branding;
    }

    pub fn set_subscription(&mut self, subscription: Subscription) {
        self.subscription = subscription;
    }

    pub fn is_in_arrears(&self, program_config: &ProgramConfig) -> bool {
        let subscription_amount = self.get_subscription_amount(program_config);

        if subscription_amount == 0 {
            return false;
        }

        let current_time = Clock::get().unwrap().unix_timestamp;
        return current_time > self.next_payment_time + 60 * 60 * 24 * 7;
    }

    pub fn get_subscription(&self) -> Subscription {
        let time = Clock::get().unwrap().unix_timestamp;
        if time >= self.subscription_live_date {
            return self.subscription;
        } else {
            self.prev_subscription
        }
    }

    pub fn increase_staker_count(&mut self) -> Result<()> {
        self.number_staked = self
            .number_staked
            .checked_add(1)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }

    pub fn decrease_staker_count(&mut self) -> Result<()> {
        self.number_staked = self
            .number_staked
            .checked_sub(1)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }

    pub fn get_subscription_amount(&self, program_config: &ProgramConfig) -> u64 {
        let mut subscription_amount: u64 = match self.get_subscription() {
            Subscription::Advanced => program_config.advanced_subscription_fee,
            Subscription::Pro => program_config.pro_subscription_fee,
            Subscription::Ultimate => program_config.ultimate_subscription_fee,
            Subscription::Custom {
                amount,
                stake_fee: _,
                unstake_fee: _,
                claim_fee: _,
            } => amount,
            _ => 0,
        };

        // if self.own_domain {
        //     subscription_amount += program_config.own_domain_fee
        // }

        if self.remove_branding {
            subscription_amount += program_config.remove_branding_fee
        }

        if self.collections.len() > 1 {
            subscription_amount +=
                (self.collections.len() as u64 - 1) * program_config.extra_collection_fee;
        }

        subscription_amount
    }
}
