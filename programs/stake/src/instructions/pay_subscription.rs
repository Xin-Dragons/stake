use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{
    constants::{SUBSCRIPTION_WALLET, USDC_MINT_PUBKEY},
    state::{ProgramConfig, Staker},
    StakeError,
};

#[derive(Accounts)]
pub struct PaySubscription<'info> {
    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Account<'info, ProgramConfig>,

    // no ownership check - anyone can pay for a subscription
    #[account(mut)]
    pub staker: Account<'info, Staker>,

    #[account(
        mut,
        associated_token::mint = usdc,
        associated_token::authority = authority
    )]
    pub usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc,
        associated_token::authority = subscription_wallet
    )]
    pub subscription_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(address = SUBSCRIPTION_WALLET)]
    pub subscription_wallet: SystemAccount<'info>,

    #[account(address = USDC_MINT_PUBKEY)]
    pub usdc: Box<Account<'info, Mint>>,

    pub authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

impl<'info> PaySubscription<'info> {
    pub fn pay_subscription_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.usdc_account.to_account_info(),
            to: self.subscription_usdc_account.to_account_info(),
            authority: self.authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn pay_subscription_handler(ctx: Context<PaySubscription>) -> Result<()> {
    let clock = Clock::get().unwrap();
    let current_time = clock.unix_timestamp;
    let grace_period = 60 * 60 * 24 * 7;
    let thirty_days = 60 * 60 * 24 * 30;
    let next_payment_time = ctx.accounts.staker.next_payment_time;
    let earliest_payment_time = next_payment_time - thirty_days + 60 * 60 * 24;
    let end_of_grace = next_payment_time + grace_period;
    msg!("Due date {}", earliest_payment_time);

    require_gt!(
        current_time,
        earliest_payment_time,
        StakeError::PaymentNotDueYet
    );

    let subscription_amount: u64 = ctx
        .accounts
        .staker
        .get_subscription_amount(ctx.accounts.program_config.as_ref());

    msg!("SUB AMOUNT {}", subscription_amount);

    require_gt!(subscription_amount, 0, StakeError::NoPaymentDue);

    transfer(ctx.accounts.pay_subscription_ctx(), subscription_amount)?;

    let staker = &mut ctx.accounts.staker;

    if current_time < end_of_grace {
        staker.next_payment_time += thirty_days;
    } else {
        staker.next_payment_time = current_time + thirty_days;
    }

    Ok(())
}
