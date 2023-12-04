use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    state::{Collection, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct AddFunds<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized,
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection.collection_mint.as_ref(),
            b"collection",
        ],
        bump = collection.bump,
        has_one = staker
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        address = collection.reward_token.unwrap() @ StakeError::InvalidRewardToken
    )]
    pub reward_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = authority
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = token_authority
    )]
    pub stake_token_vault: Account<'info, TokenAccount>,

    /// CHECK: the account is not read or written
    #[account(
        seeds = [b"STAKE", staker.key().as_ref(), b"token-authority"],
        bump = staker.token_auth_bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

impl<'info> AddFunds<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.token_account.to_account_info(),
            to: self.stake_token_vault.to_account_info(),
            authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn add_funds_handler(ctx: Context<AddFunds>, amount: u64) -> Result<()> {
    let status = ctx.accounts.staker.is_active;

    require_eq!(status, true, StakeError::StakeInactive);
    require!(
        Option::is_some(&ctx.accounts.collection.reward_token),
        StakeError::NoRewardMint
    );

    transfer(ctx.accounts.transfer_token_ctx(), amount)?;
    ctx.accounts.collection.increase_current_balance(amount)
}
