use anchor_lang::prelude::*;

use crate::StakeError;

#[account]
pub struct NftRecord {
    /// nft_mint public key (32)
    pub nft_mint: Pubkey,
    /// persistant points (8)
    pub points: u64,
    /// Bump of the NFT Record PDA (1)
    pub bump: u8,
}

impl NftRecord {
    pub const LEN: usize = 8 + 32 + 8 + 1;

    pub fn init(nft_mint: Pubkey, bump: u8) -> Self {
        Self {
            nft_mint,
            points: 0,
            bump,
        }
    }

    pub fn add_points(&mut self, points: u64) -> Result<()> {
        self.points = self
            .points
            .checked_add(points)
            .ok_or(StakeError::ProgramAddError)?;

        Ok(())
    }

    pub fn subtract_points(&mut self, points: u64) -> Result<()> {
        self.points = self
            .points
            .checked_sub(points)
            .ok_or(StakeError::ProgramSubError)?;

        Ok(())
    }
}
