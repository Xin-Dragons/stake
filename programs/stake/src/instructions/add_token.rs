use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{
        set_authority, spl_token::instruction::AuthorityType, Mint, SetAuthority, Token,
        TokenAccount,
    },
};

use crate::{
    state::{Collection, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct AddToken<'info> {
    #[account(
        mut,
        has_one = authority,
        constraint = staker.token_mint == None @ StakeError::TokenExists
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint,
        associated_token::authority = token_authority
    )]
    pub token_vault: Option<Account<'info, TokenAccount>>,

    /// CHECK: this account is not read or written to
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump = staker.token_auth_bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    pub token_program: Program<'info, Token>,

    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> AddToken<'info> {
    pub fn transfer_auth_ctx(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.token_mint.to_account_info(),
            current_authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn add_token_handler(ctx: Context<AddToken>, token_vault: bool) -> Result<()> {
    require!(
        Option::is_none(&ctx.accounts.staker.token_mint),
        StakeError::TokenExists
    );
    if token_vault {
        require!(
            Option::is_some(&ctx.accounts.token_vault),
            StakeError::TokenVaultRequired
        )
    } else {
        let mint_auth = ctx
            .accounts
            .token_mint
            .mint_authority
            .expect("mint auth has been revoked");
        let token_auth = ctx.accounts.token_authority.key();
        if !token_auth.eq(&mint_auth) {
            set_authority(
                ctx.accounts.transfer_auth_ctx(),
                AuthorityType::MintTokens,
                Some(ctx.accounts.token_authority.key()),
            )?;
        }
    }

    let staker = &mut ctx.accounts.staker;

    staker.token_mint = Some(ctx.accounts.token_mint.key());
    staker.token_vault = token_vault;

    Ok(())
}
