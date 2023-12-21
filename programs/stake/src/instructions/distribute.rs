use anchor_lang::{
    prelude::*,
    system_program::{transfer, Transfer},
};

use crate::{
    state::{Collection, Distribution, ShareRecord, StakeRecord, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct Distribute<'info> {
    #[account(
        mut,
        has_one = authority @ StakeError::Unauthorized,
    )]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        has_one = staker,
        constraint = distribution.shares_funded < distribution.num_shares @ StakeError::TotalSharesFunded
    )]
    pub distribution: Account<'info, Distribution>,

    #[account(
        init,
        payer = authority,
        space = ShareRecord::LEN,
        seeds = [
            b"STAKE",
            distribution.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"share-record"
        ],
        bump
    )]
    pub share_record: Account<'info, ShareRecord>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"stake-record",
        ],
        bump = stake_record.bump,
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    #[account(
        seeds = [
            b"STAKE",
            collection.staker.as_ref(),
            collection.collection_mint.as_ref(),
            b"collection"
        ],
        bump = collection.bump
    )]
    pub collection: Account<'info, Collection>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> Distribute<'info> {
    pub fn distribute_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.authority.to_account_info(),
            to: self.share_record.to_account_info(),
        };

        let cpi_program = self.system_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn distribute_handler(ctx: Context<Distribute>, amount: u64) -> Result<()> {
    let distribution = &ctx.accounts.distribution;

    if !Option::is_some(&distribution.token_mint) {
        let to_transfer: u64 = amount
            .checked_sub(ctx.accounts.share_record.get_lamports())
            .ok_or(StakeError::ProgramSubError)?;

        transfer(ctx.accounts.distribute_ctx(), to_transfer)?;
    } else {
    }

    let share_record = &mut ctx.accounts.share_record;

    **share_record = ShareRecord::init(
        ctx.accounts.stake_record.owner,
        ctx.accounts.distribution.key(),
        amount,
        ctx.bumps.share_record,
    );

    let distribution = &mut ctx.accounts.distribution;

    distribution.iterate_funded();
    distribution.add_to_total(amount);

    Ok(())
}
