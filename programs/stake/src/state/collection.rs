use anchor_lang::prelude::*;

use crate::StakeError;

#[account]
pub struct Collection {
    /// staker this collection belongs to (32)
    pub staker: Pubkey,
    /// MCC mint of the collection (32)
    pub collection_mint: Pubkey,
    /// optional creators (4)
    pub creators: Vec<Pubkey>,
    /// Collection custody type (1)
    pub custodial: bool,
    /// Merkle root of allowlist (1 + 32)
    pub allow_list: Option<Pubkey>,
    /// pubkey of token emission config (1 + 32)
    pub token_emission: Option<Pubkey>,
    /// pubkey of selection emission config (1 + 32)
    pub selection_emission: Option<Pubkey>,
    /// pubkey of points emission config (1 + 32)
    pub points_emission: Option<Pubkey>,
    /// pubkey of distribution emission config (1 + 32)
    pub distribution_emission: Option<Pubkey>,
    /// items can be staked, rewards accrue (1)
    pub is_active: bool,
    /// The max number of NFTs that can be staked (8)
    pub max_stakers_count: u64,
    /// The current number of NFTs staked (8)
    pub current_stakers_count: u64,
    /// Bump of the Collection PDA (1)
    pub bump: u8,
}

impl Collection {
    pub const LEN: usize =
        8 + 32 + 32 + 4 + 1 + (1 + 32) + (1 + 32) + (1 + 32) + (1 + 32) + (1 + 32) + 1 + 8 + 8 + 1;

    pub fn init(
        staker: Pubkey,
        collection_mint: Pubkey,
        custodial: bool,
        max_stakers_count: u64,
        bump: u8,
    ) -> Self {
        Self {
            staker,
            collection_mint,
            custodial,
            // todo: add this
            creators: vec![],
            // todo: add this
            allow_list: None,
            token_emission: None,
            selection_emission: None,
            points_emission: None,
            distribution_emission: None,
            is_active: false,
            max_stakers_count,
            current_stakers_count: 0,
            bump,
        }
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

    pub fn close_collection(&mut self) {
        self.is_active = false;
    }
}
