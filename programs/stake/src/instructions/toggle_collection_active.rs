use anchor_lang::prelude::*;

use crate::{
    state::{Collection, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct ToggleCollectionActive<'info> {
    #[account(
        has_one = authority @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection.collection_mint.as_ref(),
            b"collection"
        ],
        bump = collection.bump
    )]
    pub collection: Account<'info, Collection>,

    pub authority: Signer<'info>,
}

pub fn toggle_collection_active_handler(
    ctx: Context<ToggleCollectionActive>,
    active: bool,
) -> Result<()> {
    let collection = &mut ctx.accounts.collection;
    collection.is_active = active;
    Ok(())
}
