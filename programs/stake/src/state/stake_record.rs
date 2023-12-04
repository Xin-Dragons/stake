use anchor_lang::prelude::*;

#[account]
pub struct StakeRecord {
    /// owner of the NFT 32
    pub owner: Pubkey,
    /// mint of the staked NFT (32)
    pub nft_mint: Pubkey,
    /// Staking timestamp (8)
    pub staked_at: i64,
    /// Bump of the Stake Record PDA (1)
    pub bump: u8,
}

impl StakeRecord {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 1;

    pub fn init(owner: Pubkey, nft_mint: Pubkey, staked_at: i64, bump: u8) -> Self {
        Self {
            owner,
            nft_mint,
            staked_at,
            bump,
        }
    }
}
