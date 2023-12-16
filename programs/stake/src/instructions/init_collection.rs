use anchor_lang::prelude::*;
use anchor_spl::token::{
    set_authority, spl_token::instruction::AuthorityType, Mint, SetAuthority, Token,
};

use crate::{
    state::{collection, Collection, ProgramConfig, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct InitCollection<'info> {
    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        realloc = staker.current_len() + 32,
        realloc::payer = authority,
        realloc::zero = false,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Box<Account<'info, Staker>>,

    #[account(
        init,
        payer = authority,
        space = Collection::LEN,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection_mint.key().as_ref(),
            b"collection",
        ],
        bump
    )]
    pub collection: Box<Account<'info, Collection>>,

    /// CHECK: this can either be a collection or a creator
    pub collection_mint: UncheckedAccount<'info>,

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

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn init_collection_handler(
    ctx: Context<InitCollection>,
    custodial: bool,
    staking_starts_at: Option<i64>,
    max_stakers_count: u64,
) -> Result<()> {
    let clock = Clock::get().unwrap();
    let current_time = clock.unix_timestamp;

    let start_time = staking_starts_at.unwrap_or(current_time);

    require_gte!(start_time, current_time, StakeError::StartTimeInPast);

    let collection = &mut ctx.accounts.collection;

    ***collection = Collection::init(
        ctx.accounts.staker.key(),
        ctx.accounts.collection_mint.key(),
        custodial,
        max_stakers_count,
        ctx.bumps.collection,
    );

    let staker: &mut Account<'_, Staker> = &mut ctx.accounts.staker;
    staker.add_collection(ctx.accounts.collection.key())
}
