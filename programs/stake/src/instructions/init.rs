use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};
use proc_macro_regex::regex;
use rustrict::CensorStr;

use crate::{
    constants::{SUBSCRIPTION_WALLET, USDC_MINT_PUBKEY},
    state::{ProgramConfig, Staker, Subscription, Theme},
    StakeError,
};

regex!(regex_slug "^(?:[_a-z0-9]+)*$");

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(
        mut,
        seeds = [b"program-config"],
        bump,
        realloc = program_config.current_len() + 50,
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub program_config: Account<'info, ProgramConfig>,

    #[account(
        init,
        payer = authority,
        space = Staker::LEN,
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = usdc,
        associated_token::authority = authority
    )]
    pub usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = usdc,
        associated_token::authority = subscription_wallet
    )]
    pub subscription_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(address = SUBSCRIPTION_WALLET)]
    pub subscription_wallet: SystemAccount<'info>,

    #[account(address = USDC_MINT_PUBKEY)]
    pub usdc: Box<Account<'info, Mint>>,

    /// CHECK: This account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    /// CHECK: This account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"nft-authority"
        ],
        bump
    )]
    pub nft_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Init<'info> {
    pub fn transfer_subscription_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.usdc_account.to_account_info(),
            to: self.subscription_usdc_account.to_account_info(),
            authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn init_handler(
    ctx: Context<Init>,
    slug: String,
    name: String,
    remove_branding: bool,
    own_domain: bool,
    subscription: Option<Subscription>,
    start_date: i64,
) -> Result<()> {
    require_gte!(50, slug.len(), StakeError::SlugTooLong);
    require_gt!(slug.len(), 0, StakeError::SlugRequired);

    let program_config = &mut ctx.accounts.program_config;

    let existing_slugs = &program_config.slugs;
    require!(!existing_slugs.contains(&slug), StakeError::SlugExists);

    require_gte!(50, name.len(), StakeError::NameTooLong);
    require_gt!(name.len(), 0, StakeError::NameRequired);

    require!(!name.is_inappropriate(), StakeError::ProfanityDetected);

    require!(regex_slug(&slug), StakeError::InvalidSlug);

    program_config.slugs.push(slug.clone());

    let creator = ctx.accounts.authority.key();
    let token_auth_bump = ctx.bumps.token_authority;
    let nft_auth_bump = ctx.bumps.nft_authority;

    let clock = Clock::get().unwrap();
    let current_time = clock.unix_timestamp;

    let actual_start_time = if start_date >= current_time {
        start_date
    } else {
        current_time
    };

    let staker = &mut ctx.accounts.staker;

    **staker = Staker::init(
        slug,
        name,
        creator,
        remove_branding,
        own_domain,
        subscription.unwrap_or(Subscription::Free),
        token_auth_bump,
        nft_auth_bump,
        actual_start_time,
        actual_start_time + 60 * 60 * 24 * 30,
    );

    staker.theme = Theme::default();

    let subscription_amount = staker.get_subscription_amount(&ctx.accounts.program_config);

    if subscription_amount > 0 {
        transfer(
            ctx.accounts.transfer_subscription_ctx(),
            subscription_amount,
        )?;
    }

    Ok(())
}
