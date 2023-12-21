use anchor_lang::prelude::*;

use crate::state::Staker;

#[derive(Accounts)]
pub struct ToggleStakeActive<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub staker: Account<'info, Staker>,

    pub authority: Signer<'info>,
}

pub fn toggle_stake_active_handler(ctx: Context<ToggleStakeActive>, is_active: bool) -> Result<()> {
    let stake = &mut ctx.accounts.staker;

    stake.is_active = is_active;

    Ok(())
}
