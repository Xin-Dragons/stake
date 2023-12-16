use anchor_lang::prelude::*;

use crate::{
    state::{Collection, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct CloseCollection<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized
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
        close = authority,
        bump = collection.bump,
    )]
    pub collection: Account<'info, Collection>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn close_collection_handler(ctx: Context<CloseCollection>) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;

    let Collection {
        current_stakers_count,
        ..
    } = **collection;

    let has_emissions = Option::is_some(&collection.token_emission)
        || Option::is_some(&collection.points_emission)
        || Option::is_some(&collection.distribution_emission)
        || Option::is_some(&collection.selection_emission);

    require!(!has_emissions, StakeError::StillHasEmissions);

    require_eq!(current_stakers_count, 0, StakeError::CollectionHasStakers);

    let collection = &mut ctx.accounts.collection;

    collection.close_collection();

    let collections: Vec<Pubkey> = staker.collections.clone();

    let staker: &mut Account<'_, Staker> = &mut ctx.accounts.staker;

    staker.collections = collections
        .into_iter()
        .filter(|coll| coll.to_bytes() != ctx.accounts.collection.key().to_bytes())
        .collect();

    Ok(())
}
