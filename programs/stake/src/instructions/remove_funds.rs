use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    state::{Collection, Emission, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct RemoveFunds<'info> {
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

    #[account(mut, has_one = collection)]
    pub emission: Account<'info, Emission>,

    #[account(
        address = emission.token_mint.unwrap() @ StakeError::InvalidRewardToken
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

impl<'info> RemoveFunds<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.stake_token_vault.to_account_info(),
            to: self.token_account.to_account_info(),
            authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn remove_funds_handler(ctx: Context<RemoveFunds>) -> Result<()> {
    let staker_key = &ctx.accounts.staker.key();
    let token_auth_bump = ctx.accounts.staker.token_auth_bump;
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;

    require_eq!(collection.is_active, false, StakeError::CollectionActive);
    require!(
        Option::is_some(&ctx.accounts.emission.token_mint),
        StakeError::NoRewardMint
    );

    require_gt!(emission.current_balance, 0, StakeError::NoTokensToClaim);

    require_eq!(
        ctx.accounts.collection.current_stakers_count,
        0,
        StakeError::CollectionHasStakers
    );

    let token_auth_seed = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"token-authority"[..],
        &[token_auth_bump],
    ];

    transfer(
        ctx.accounts
            .transfer_token_ctx()
            .with_signer(&[&token_auth_seed[..]]),
        emission.current_balance,
    )?;

    ctx.accounts.emission.current_balance = 0;

    Ok(())
}
