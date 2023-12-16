use anchor_lang::prelude::*;
use anchor_spl::token::{
    set_authority, spl_token::instruction::AuthorityType, Mint, SetAuthority, Token,
};
use solana_program::stake;

use crate::{
    state::{ProgramConfig, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(
        mut,
        seeds = [
            b"program-config"
        ],
        bump
    )]
    pub program_config: Account<'info, ProgramConfig>,

    #[account(
        mut,
        close = authority,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub token_mint: Option<Account<'info, Mint>>,

    /// CHECK: this account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump = staker.token_auth_bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Close<'info> {
    pub fn transfer_auth_ctx(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self
                .token_mint
                .as_ref()
                .expect("token_mint expected")
                .to_account_info(),
            current_authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn close_handler(ctx: Context<Close>) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let token_mint = &ctx.accounts.token_mint;
    let staker_key = staker.key();
    let token_auth_bump = staker.token_auth_bump;
    let program_config = &ctx.accounts.program_config;

    require_eq!(staker.collections.len(), 0, StakeError::StillHasCollections);
    require_eq!(staker.number_staked, 0, StakeError::StillHasStakedItems);

    if Option::is_some(&staker.token_mint) && !staker.token_vault {
        let token_auth_seed: &[&[u8]; 4] = &[
            &b"STAKE"[..],
            &staker_key.as_ref(),
            &b"token-authority"[..],
            &[token_auth_bump],
        ];

        let mint_auth = token_mint
            .as_ref()
            .map(|o| o.mint_authority)
            .expect("token_mint expected")
            .unwrap();

        let token_auth = ctx.accounts.token_authority.key();
        require_keys_eq!(mint_auth, token_auth, StakeError::NoAuthority);
        set_authority(
            ctx.accounts
                .transfer_auth_ctx()
                .with_signer(&[&token_auth_seed[..]]),
            AuthorityType::MintTokens,
            Some(ctx.accounts.authority.key()),
        )?;
    }

    let slugs: Vec<String> = program_config.slugs.clone();

    let program_config = &mut ctx.accounts.program_config;

    program_config.slugs = slugs
        .into_iter()
        .filter(|slug| slug != &staker.slug)
        .collect();

    Ok(())
}
