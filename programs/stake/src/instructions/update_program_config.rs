use anchor_lang::prelude::*;

use crate::{program::Stake, state::ProgramConfig, StakeError};

#[derive(Accounts)]
pub struct UpdateProgramConfig<'info> {
    #[account(
        mut,
        seeds = [
            b"program-config"
        ],
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
}

pub fn clear_slugs_handler(ctx: Context<UpdateProgramConfig>) -> Result<()> {
    let program_config = &mut ctx.accounts.program_config;

    program_config.slugs = vec![];
    Ok(())
}

pub fn update_program_config_handler(
    ctx: Context<UpdateProgramConfig>,
    stake_fee: Option<u64>,
    unstake_fee: Option<u64>,
    claim_fee: Option<u64>,
    advanced_subscription_fee: Option<u64>,
    pro_subscription_fee: Option<u64>,
    ultimate_subscription_fee: Option<u64>,
    extra_collection_fee: Option<u64>,
    remove_branding_fee: Option<u64>,
    own_domain_fee: Option<u64>,
) -> Result<()> {
    let program_config = &mut ctx.accounts.program_config;

    program_config.stake_fee = stake_fee.unwrap_or(program_config.stake_fee);
    program_config.unstake_fee = unstake_fee.unwrap_or(program_config.unstake_fee);
    program_config.claim_fee = claim_fee.unwrap_or(program_config.claim_fee);
    program_config.advanced_subscription_fee =
        advanced_subscription_fee.unwrap_or(program_config.advanced_subscription_fee);
    program_config.pro_subscription_fee =
        pro_subscription_fee.unwrap_or(program_config.pro_subscription_fee);
    program_config.ultimate_subscription_fee =
        ultimate_subscription_fee.unwrap_or(program_config.ultimate_subscription_fee);
    program_config.extra_collection_fee =
        extra_collection_fee.unwrap_or(program_config.extra_collection_fee);
    program_config.remove_branding_fee =
        remove_branding_fee.unwrap_or(program_config.remove_branding_fee);
    program_config.own_domain_fee = own_domain_fee.unwrap_or(program_config.own_domain_fee);

    Ok(())
}
