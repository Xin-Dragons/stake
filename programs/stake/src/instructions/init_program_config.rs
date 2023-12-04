use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{
    constants::{SUBSCRIPTION_WALLET, USDC_MINT_PUBKEY},
    program::Stake,
    state::ProgramConfig,
    StakeError,
};

#[derive(Accounts)]
pub struct InitProgramConfig<'info> {
    #[account(
        init,
        space = ProgramConfig::LEN,
        payer = authority,
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Account<'info, ProgramConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        constraint = program.programdata_address()? == Some(program_data.key()) @ StakeError::AdminOnly
    )]
    pub program: Program<'info, Stake>,

    #[account(
        constraint = program_data.upgrade_authority_address == Some(authority.key()) @ StakeError::AdminOnly
    )]
    pub program_data: Account<'info, ProgramData>,

    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = usdc,
        associated_token::authority = subscription_wallet
    )]
    pub subscription_usdc_account: Box<Account<'info, TokenAccount>>,

    #[account(address = SUBSCRIPTION_WALLET)]
    pub subscription_wallet: SystemAccount<'info>,

    #[account(address = USDC_MINT_PUBKEY)]
    pub usdc: Box<Account<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn init_program_config_handler(
    ctx: Context<InitProgramConfig>,
    stake_fee: u64,
    unstake_fee: u64,
    claim_fee: u64,
    advanced_subscription_fee: u64,
    pro_subscription_fee: u64,
    ultimate_subscription_fee: u64,
    extra_collection_fee: u64,
    remove_branding_fee: u64,
    own_domain_fee: u64,
) -> Result<()> {
    let program_config = &mut ctx.accounts.program_config;
    let bump = ctx.bumps.program_config;
    **program_config = ProgramConfig::init(
        stake_fee,
        unstake_fee,
        claim_fee,
        advanced_subscription_fee,
        pro_subscription_fee,
        ultimate_subscription_fee,
        extra_collection_fee,
        remove_branding_fee,
        own_domain_fee,
        bump,
    );

    Ok(())
}
