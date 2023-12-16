use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    state::{Collection, Emission, RewardType, Staker},
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
        mut,
        has_one = collection
    )]
    pub emission: Account<'info, Emission>,

    #[account(
        // address = emission.reward_type.reward_token.unwrap() @ StakeError::InvalidRewardToken
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
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;
    let status = ctx.accounts.staker.is_active;

    require_eq!(status, true, StakeError::StakeInactive);

    require_keys_eq!(
        collection.token_emission.expect("token_emission expected"),
        emission.key(),
        StakeError::InvalidEmission
    );

    match emission.reward_type {
        RewardType::Token => {}
        RewardType::Selection { options: _ } => {}
        _ => {
            return err!(StakeError::InvalidEmission);
        }
    }

    require_keys_eq!(
        emission.token_mint.unwrap(),
        ctx.accounts.reward_mint.key(),
        StakeError::InvalidRewardToken
    );

    require_keys_neq!(
        ctx.accounts.reward_mint.key(),
        ctx.accounts.staker.token_mint.unwrap_or(Pubkey::default()),
        StakeError::InvalidEmission
    );

    transfer(ctx.accounts.transfer_token_ctx(), amount)?;
    let emission = &mut ctx.accounts.emission;
    emission.increase_current_balance(amount)
}
