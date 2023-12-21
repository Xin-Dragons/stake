use anchor_lang::prelude::*;

#[account]
pub struct ShareRecord {
    /// owner of the share record (32)
    pub owner: Pubkey,
    /// distribution this record belongs to (32)
    pub distribution: Pubkey,
    /// amount in basis points (8)
    pub amount: u64,
    /// bump of the share_record account (1)
    pub bump: u8,
}

impl ShareRecord {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 1;

    pub fn init(owner: Pubkey, distribution: Pubkey, amount: u64, bump: u8) -> Self {
        Self {
            owner,
            distribution,
            amount,
            bump,
        }
    }
}
