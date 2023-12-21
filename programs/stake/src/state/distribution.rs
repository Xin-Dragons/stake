use anchor_lang::prelude::*;

use super::collection;

#[account]
pub struct Distribution {
    /// staker this distribution belongs to (32)
    pub staker: Pubkey,
    /// collection this distribution belongs to (32)
    pub collection: Pubkey,
    /// descriptor (4 + 20)
    pub label: String,
    /// token mint, if not sol
    pub token_mint: Option<Pubkey>,
    /// uri link to offchain distribution log (4 + 63)
    pub uri: String,
    /// total amount of this distribution (8)
    pub total_amount: u64,
    /// current balance of this distribution (8)
    pub balance: u64,
    /// total number of shares (4)
    pub num_shares: u32,
    /// number of funded shares (4)
    pub shares_funded: u32,
    /// timestamp (8)
    pub created_at: i64,
    /// amount claimed (8)
    pub claimed_amount: u64,
    /// have all shares been assigned? (1)
    pub complete: bool,
    /// can users claim? (1)
    pub active: bool,
    /// bump of the vault authority
    pub vault_authority_bump: u8,
}

impl Distribution {
    pub const LEN: usize =
        8 + 32 + (4 + 20) + (1 + 32) + (4 + 63) + 8 + 8 + 4 + 4 + 8 + 8 + 1 + 1 + 1;

    pub fn init(
        staker: Pubkey,
        token_mint: Option<Pubkey>,
        collection: Pubkey,
        label: String,
        uri: String,
        num_shares: u32,
        created_at: i64,
        amount: u64,
        vault_authority_bump: u8,
    ) -> Self {
        Self {
            staker,
            collection,
            label,
            uri,
            token_mint,
            num_shares,
            shares_funded: 0,
            claimed_amount: 0,
            created_at,
            complete: false,
            active: false,
            balance: amount,
            total_amount: amount,
            vault_authority_bump,
        }
    }

    pub fn iterate_funded(&mut self) {
        self.shares_funded += 1;
        if self.shares_funded >= self.num_shares {
            self.complete = true;
        }
    }

    pub fn add_to_claimed(&mut self, amount: u64) {
        self.claimed_amount += amount;
        self.balance -= amount;
    }

    pub fn add_to_total(&mut self, amount: u64) {
        self.total_amount += amount;
        self.balance += amount;
    }
}
