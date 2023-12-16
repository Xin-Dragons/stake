use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};

use crate::{
    constants::{SUBSCRIPTION_WALLET, USDC_MINT_PUBKEY},
    program::Stake,
    state::{ProgramConfig, Staker, Subscription},
    utils::calc_pro_rata_fee,
    StakeError,
};

#[derive(Accounts)]
pub struct UpdateStakeAdmin<'info> {
    #[account(mut)]
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
}

#[derive(Accounts)]
pub struct UpdateStake<'info> {
    #[account(
        seeds = [
            b"program-config"
        ],
        bump
    )]
    pub program_config: Account<'info, ProgramConfig>,

    #[account(
        mut,
        constraint = signer.key() == staker.authority ||
            Some(signer.key()) == program_data.as_ref().expect("program data must be provided if not stake authority").upgrade_authority_address
            @ StakeError::Unauthorized
    )]
    pub staker: Account<'info, Staker>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(address = USDC_MINT_PUBKEY)]
    pub usdc: Option<Box<Account<'info, Mint>>>,

    #[account(
        mut,
        associated_token::mint = usdc,
        associated_token::authority = signer
    )]
    pub usdc_account: Option<Box<Account<'info, TokenAccount>>>,

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = usdc,
        associated_token::authority = subscription_wallet
    )]
    pub subscription_usdc_account: Option<Box<Account<'info, TokenAccount>>>,

    #[account(address = SUBSCRIPTION_WALLET)]
    pub subscription_wallet: Option<SystemAccount<'info>>,

    #[account(
        constraint = program.programdata_address()? ==
            Some(program_data.as_ref().unwrap().key())
            @ StakeError::InvalidProgramData
    )]
    pub program: Option<Program<'info, Stake>>,

    #[account()]
    pub program_data: Option<Account<'info, ProgramData>>,

    pub system_program: Option<Program<'info, System>>,
    pub token_program: Option<Program<'info, Token>>,
    pub associated_token_program: Option<Program<'info, AssociatedToken>>,
}

impl<'info> UpdateStake<'info> {
    pub fn transfer_subscription_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .usdc_account
                .as_ref()
                .expect("usdc_account expected")
                .to_account_info(),
            to: self
                .subscription_usdc_account
                .as_ref()
                .expect("subscription_usdc_account expected")
                .to_account_info(),
            authority: self.signer.to_account_info(),
        };

        let cpi_program = self
            .token_program
            .as_ref()
            .expect("token_program expected")
            .to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

/// Admin only handler
pub fn update_stake_next_payment_time_handler(
    ctx: Context<UpdateStakeAdmin>,
    next_payment_time: i64,
) -> Result<()> {
    ctx.accounts.staker.next_payment_time = next_payment_time;
    Ok(())
}

pub fn update_stake_subscription_handler(
    ctx: Context<UpdateStake>,
    subscription: Subscription,
) -> Result<()> {
    let current_time = Clock::get().unwrap().unix_timestamp;
    let program_data = &ctx.accounts.program_data.as_ref();
    if Option::is_none(program_data)
        || program_data.unwrap().upgrade_authority_address != Some(ctx.accounts.signer.key())
    {
        // only system admin can set a custom subscription
        match subscription {
            Subscription::Custom {
                amount: _,
                stake_fee: _,
                unstake_fee: _,
                claim_fee: _,
            } => return err!(StakeError::Unauthorized),
            _ => {}
        }
        if ctx
            .accounts
            .staker
            .is_in_arrears(&ctx.accounts.program_config)
        {
            return err!(StakeError::StakeInArrears);
        }

        let fee: u64 = match subscription {
            Subscription::Advanced => ctx.accounts.program_config.advanced_subscription_fee,
            Subscription::Pro => ctx.accounts.program_config.pro_subscription_fee,
            Subscription::Ultimate => ctx.accounts.program_config.ultimate_subscription_fee,
            _ => 0,
        };

        let current_fee = match ctx.accounts.staker.subscription {
            Subscription::Advanced => ctx.accounts.program_config.advanced_subscription_fee,
            Subscription::Pro => ctx.accounts.program_config.pro_subscription_fee,
            Subscription::Ultimate => ctx.accounts.program_config.ultimate_subscription_fee,
            Subscription::Custom {
                amount,
                stake_fee: _,
                unstake_fee: _,
                claim_fee: _,
            } => amount,
            _ => 0,
        };

        if fee > current_fee {
            let fee_payable = calc_pro_rata_fee(ctx.accounts.staker.next_payment_time, fee)?;
            if fee_payable > 0 {
                transfer(ctx.accounts.transfer_subscription_ctx(), fee_payable)?;
            }
        } else {
            ctx.accounts.staker.subscription_live_date = ctx.accounts.staker.next_payment_time;
            ctx.accounts.staker.prev_subscription = ctx.accounts.staker.subscription;
        }
    }
    ctx.accounts.staker.next_payment_time = current_time + 60 * 60 * 24 * 30;
    ctx.accounts.staker.subscription = subscription;
    Ok(())
}

pub fn update_stake_remove_branding_handler(
    ctx: Context<UpdateStake>,
    remove_branding: bool,
) -> Result<()> {
    let program_data = &ctx.accounts.program_data.as_ref();
    if Option::is_none(program_data)
        || program_data.unwrap().upgrade_authority_address != Some(ctx.accounts.signer.key())
    {
        if ctx
            .accounts
            .staker
            .is_in_arrears(&ctx.accounts.program_config)
        {
            return err!(StakeError::StakeInArrears);
        }

        let fee: u64 = if remove_branding {
            ctx.accounts.program_config.remove_branding_fee
        } else {
            0
        };

        // only charge if they didn't have it before
        if !ctx.accounts.staker.remove_branding {
            let fee_payable = calc_pro_rata_fee(ctx.accounts.staker.next_payment_time, fee)?;
            if fee_payable > 0 {
                transfer(ctx.accounts.transfer_subscription_ctx(), fee_payable)?;
            }
        }
    }
    ctx.accounts.staker.remove_branding = remove_branding;
    Ok(())
}

pub fn update_stake_own_domain_handler(ctx: Context<UpdateStake>, own_domain: bool) -> Result<()> {
    let program_data = &ctx.accounts.program_data.as_ref();
    if Option::is_none(program_data)
        || program_data.unwrap().upgrade_authority_address != Some(ctx.accounts.signer.key())
    {
        if ctx
            .accounts
            .staker
            .is_in_arrears(&ctx.accounts.program_config)
        {
            return err!(StakeError::StakeInArrears);
        }

        let fee: u64 = if own_domain {
            ctx.accounts.program_config.own_domain_fee
        } else {
            0
        };

        // only charge if they didn't have it before
        if !ctx.accounts.staker.own_domain {
            let fee_payable = calc_pro_rata_fee(ctx.accounts.staker.next_payment_time, fee)?;
            if fee_payable > 0 {
                transfer(ctx.accounts.transfer_subscription_ctx(), fee_payable)?;
            }
        }
    }
    ctx.accounts.staker.own_domain = own_domain;
    Ok(())
}
