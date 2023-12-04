use anchor_lang::prelude::*;

use crate::{state::Staker, StakeError};

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(
        mut,
        close = authority,
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    pub authority: Signer<'info>,
}

pub fn close_handler(ctx: Context<Close>) -> Result<()> {
    let staker = &mut ctx.accounts.staker;

    require_eq!(staker.collections.len(), 0, StakeError::StillHasCollections);
    require_eq!(staker.number_staked, 0, StakeError::StillHasStakedItems);

    Ok(())
}
