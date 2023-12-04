use anchor_lang::prelude::*;

#[account]
pub struct ProgramConfig {
    /// tx fee for staking (8)
    pub stake_fee: u64,
    /// tx fee for unstaking (8)
    pub unstake_fee: u64,
    /// tx fee for claiming (8)
    pub claim_fee: u64,
    /// monthly fee for advanced (8)
    pub advanced_subscription_fee: u64,
    /// monthly fee for pro (8)
    pub pro_subscription_fee: u64,
    /// monthly fee for ultimate (8)
    pub ultimate_subscription_fee: u64,
    /// monthly fee for additional collections (8)
    pub extra_collection_fee: u64,
    /// monthly fee for removing branding (8)
    pub remove_branding_fee: u64,
    /// monthly fee for own domain (8)
    pub own_domain_fee: u64,
    /// a vector storing all slugs (4)
    pub slugs: Vec<String>,
    /// bump for the program config account (1)
    pub bump: u8,
}

impl ProgramConfig {
    pub const LEN: usize = 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + 4 + 1;

    pub fn init(
        stake_fee: u64,
        unstake_fee: u64,
        claim_fee: u64,
        advanced_subscription_fee: u64,
        pro_subscription_fee: u64,
        ultimate_subscription_fee: u64,
        extra_collection_fee: u64,
        remove_branding_fee: u64,
        own_domain_fee: u64,
        bump: u8,
    ) -> Self {
        Self {
            stake_fee,
            unstake_fee,
            claim_fee,
            advanced_subscription_fee,
            pro_subscription_fee,
            ultimate_subscription_fee,
            extra_collection_fee,
            remove_branding_fee,
            own_domain_fee,
            slugs: vec![],
            bump,
        }
    }

    pub fn current_len(&self) -> usize {
        ProgramConfig::LEN + 50 * self.slugs.len()
    }
}
