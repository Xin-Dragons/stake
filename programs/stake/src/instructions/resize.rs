use anchor_lang::prelude::*;

use crate::{program::Stake, state::Staker, StakeError};

#[derive(Accounts)]
pub struct Resize<'info> {
    #[account(
        mut,
        realloc = staker.current_len() + staker.theme.current_len(),
        realloc::payer = authority,
        realloc::zero = false
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        constraint = program.programdata_address()? == Some(program_data.key()) @  StakeError::AdminOnly
    )]
    pub program: Program<'info, Stake>,

    #[account(
        constraint = program_data.upgrade_authority_address == Some(authority.key()) @ StakeError::AdminOnly
    )]
    pub program_data: Account<'info, ProgramData>,

    pub system_program: Program<'info, System>,
}

pub fn resize_handler(_ctx: Context<Resize>) -> Result<()> {
    Ok(())
}
