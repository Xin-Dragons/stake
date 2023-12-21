use anchor_lang::prelude::*;

use crate::state::Staker;

#[derive(Accounts)]
pub struct DelegateStake<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub stake: Account<'info, Staker>,

    pub delegate: SystemAccount<'info>,

    pub authority: Signer<'info>,
}

pub fn handle_delegate_stake(ctx: Context<DelegateStake>) -> Result<()> {
    let stake = &mut ctx.accounts.stake;

    stake.authority = ctx.accounts.delegate.key();

    Ok(())
}
