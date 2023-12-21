use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, Transfer},
};

use crate::{
    state::{Collection, Distribution, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct InitDistribution<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        has_one = staker
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        init,
        space = Distribution::LEN,
        payer = authority
    )]
    pub distribution: Account<'info, Distribution>,

    #[account()]
    pub token_mint: Option<Account<'info, Mint>>,

    #[account(
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Option<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = vault_authority
    )]
    pub token_vault: Option<Account<'info, TokenAccount>>,

    /// CHECK: this account is not read or written to
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            distribution.key().as_ref(),
            b"distribution-vault"
        ],
        bump
    )]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub token_program: Program<'info, Token>,

    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> InitDistribution<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .token_account
                .as_ref()
                .expect("token_account expected")
                .to_account_info(),
            to: self
                .token_vault
                .as_ref()
                .expect("token_vault expected")
                .to_account_info(),

            authority: self.vault_authority.as_ref().to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn init_distribution_handler(
    ctx: Context<InitDistribution>,
    label: String,
    uri: String,
    num_shares: u32,
    amount: u64,
) -> Result<()> {
    let current_time = Clock::get().unwrap().unix_timestamp;

    require_gte!(20, label.len(), StakeError::LabelTooLong);

    require_gte!(amount, 0, StakeError::AmountTooLow);

    let distribution = &mut ctx.accounts.distribution;

    **distribution = Distribution::init(
        ctx.accounts.staker.key(),
        ctx.accounts.token_mint.as_ref().map(|a| a.key()),
        ctx.accounts.collection.key(),
        label,
        uri,
        num_shares,
        current_time,
        amount,
        ctx.bumps.vault_authority,
    );

    Ok(())
}
