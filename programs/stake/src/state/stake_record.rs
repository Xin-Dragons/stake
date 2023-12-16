use anchor_lang::prelude::*;

use crate::StakeError;

#[account]
pub struct StakeRecord {
    /// staker that this record belongs to (32)
    pub staker: Pubkey,
    /// owner of the NFT 32
    pub owner: Pubkey,
    /// mint of the staked NFT (32)
    pub nft_mint: Pubkey,
    /// emissions (4 + 32 * 4),
    pub emissions: Vec<Pubkey>,
    /// pending token balance to claim (8)
    pub pending_claim: u64,
    /// timestamp of when can claim (8)
    pub can_claim_at: i64,
    /// sol balance to claim (8)
    pub sol_balance: u64,
    /// Staking timestamp (8)
    pub staked_at: i64,
    /// Bump of the Stake Record PDA (1)
    pub bump: u8,
}

impl StakeRecord {
    pub const LEN: usize = 8 + 32 + 32 + (4 + 32 * 4) + 32 + 32 + 8 + 8 + 8 + 32 + 8 + 8 + 1;

    pub fn init(
        staker: Pubkey,
        owner: Pubkey,
        nft_mint: Pubkey,
        emissions: Vec<Pubkey>,
        staked_at: i64,
        pending_claim: u64,
        can_claim_at: i64,
        bump: u8,
    ) -> Self {
        Self {
            staker,
            owner,
            nft_mint,
            emissions,
            staked_at,
            pending_claim,
            can_claim_at,
            sol_balance: 0,
            bump,
        }
    }

    pub fn add_sol(&mut self, added_funds: u64) -> Result<()> {
        self.sol_balance = self
            .sol_balance
            .checked_add(added_funds)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }
}
