use anchor_lang::prelude::*;
use anchor_spl::{
    token::{ Mint, TokenAccount, Token, Transfer, MintTo, mint_to, transfer},
    associated_token::AssociatedToken,
};
use solana_program::program_option::COption;

use crate::{ state::{ Staker, Collection, ProgramConfig, StakeRecord, RewardType, NftRecord, Subscription, Emission }, StakeError, utils::{ calc_tx_fee, calc_reward}, constants::FEES_WALLET, STAKING_ENDS };

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Box<Account<'info, ProgramConfig>>,

    #[account(mut)]
    pub staker: Box<Account<'info, Staker>>,

    #[account(
        mut, 
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            collection.collection_mint.as_ref(),
            b"collection",
        ],
        bump = collection.bump,
    )]
    pub collection: Box<Account<'info, Collection>>,

    #[account(
        mut,
        has_one = collection
    )]
    pub emission: Box<Account<'info, Emission>>,

    #[account(
        mut, 
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"stake-record",
        ],
        bump = stake_record.bump,
        has_one = owner @ StakeError::Unauthorized,
        constraint = stake_record.emissions.contains(&emission.key())
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    #[account(
        mut, 
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"nft-record",
        ],
        bump = nft_record.bump
    )]
    pub nft_record: Option<Box<Account<'info, NftRecord>>>,

    #[account(mut, address = FEES_WALLET)]
    pub fees_wallet: SystemAccount<'info>,

    #[account(
        mut,
        address = staker.token_mint.unwrap()
    )]
    pub token_mint: Option<Box<Account<'info, Mint>>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_authority
    )]
    pub stake_token_vault: Option<Box<Account<'info, TokenAccount>>>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = token_mint,
        associated_token::authority = owner
    )]
    pub reward_receive_account: Option<Box<Account<'info, TokenAccount>>>,

    /// CHECK: this account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump = staker.token_auth_bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Claim<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.stake_token_vault.as_ref().expect("stake_token_vault expected").to_account_info(),
            to: self.reward_receive_account.as_ref().expect("reward_receive_account expected").to_account_info(),
            authority: self.token_authority.as_ref().to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn mint_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.token_mint.as_ref().expect("token_mint expected").to_account_info(),
            to: self.reward_receive_account.as_ref().expect("reward_receive_account expected").to_account_info(),
            authority: self.token_authority.as_ref().to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn claim_handler(ctx: Context<Claim>) -> Result<()> {
    let current_time = Clock::get().unwrap().unix_timestamp;
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let emission = &ctx.accounts.emission;
    let claimer = &ctx.accounts.owner;
    let staker_key = staker.key();
    let reward_type = emission.reward_type.clone();
    let stake_record = &ctx.accounts.stake_record;
    let amount = stake_record.sol_balance;

    let Staker {
        is_active: staking_status,
        token_auth_bump,
        ..
    } = ***staker;

    require_eq!(staking_status, true, StakeError::StakeInactive);

    let Collection {
        is_active: collection_status,
        ..
    } = ***collection;

    require_eq!(collection_status, true, StakeError::CollectionInactive);
    
    let authority_seed = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"token-authority"[..],
        &[token_auth_bump],
    ];

    let binding = &[&authority_seed[..]];

    let stake_record = &mut ctx.accounts.stake_record;
    let emission = &mut ctx.accounts.emission;
    let nft_record = &mut ctx.accounts.nft_record;

    let StakeRecord { staked_at, .. } = ***stake_record;
    let Emission {
        end_time,
        minimum_period,
        ..
    } = ***emission;

    let reward_record = &emission.reward;
    let reward_change_time_record = &emission.reward_change_time;

    let (mut reward_tokens, current_time, is_eligible_for_reward) = calc_reward(
        staked_at,
        minimum_period.unwrap_or(0),
        reward_record,
        reward_change_time_record,
        end_time,
    )
    .unwrap();

    if !is_eligible_for_reward {
        return err!(StakeError::MinimumPeriodNotReached);
    }

    match reward_type {
        RewardType::Token => {
            require_gte!(
                end_time.unwrap_or(STAKING_ENDS),
                staked_at,
                StakeError::StakeOver
            );
        }
        RewardType::Points => {
            nft_record
                .as_mut()
                .expect("Nft record expected")
                .add_points(reward_tokens)?;
        }
        RewardType::Selection { options: _ } => {
            require_gte!(
                current_time,
                stake_record.can_claim_at,
                StakeError::MinimumPeriodNotReached
            );
            reward_tokens = stake_record.pending_claim;
        }
        _ => {}
    };

    let is_token = match &emission.reward_type {
        RewardType::Token => true,
        RewardType::Selection { options: _ } => true,
        _ => false,
    };

    if is_token && reward_tokens > 0 {
        if staker.token_vault {
            emission.decrease_current_balance(staked_at, current_time)?;
        }

        if current_time < emission.end_time.unwrap_or(STAKING_ENDS) {
            emission.update_staked_weight(staked_at, false)?;
            emission.update_staked_weight(current_time, true)?;
        }
    }
    
    let tx_fee = match staker.get_subscription() {
        Subscription::Custom { amount: _, stake_fee: _, unstake_fee: _, claim_fee } => claim_fee,
        _ => ctx.accounts.program_config.claim_fee
    };

    let tx_fee = calc_tx_fee(staker, tx_fee);

    if tx_fee > 0 {
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.owner.key(),
            &ctx.accounts.fees_wallet.key(),
            tx_fee
        );
    
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.fees_wallet.to_account_info(),
            ],
        )?;
    }

    match reward_type {
        RewardType::Distribution => {
            stake_record.sub_lamports(amount)?;
            claimer.add_lamports(amount)?;
            stake_record.sol_balance = 0;
        }
        _ => {}
    }

    if is_token && reward_tokens > 0 {
        if staker.token_vault {
            transfer(ctx.accounts.transfer_token_ctx().with_signer(binding), reward_tokens)?;
        } else {
            mint_to(ctx.accounts.mint_token_ctx().with_signer(binding), reward_tokens)?;
        }
    }

    // distribution type stakers should not be reset, so as
    // to not lose eligible for reward status if min period.
    match reward_type {
        RewardType::Distribution => {},
        _ => {
            ctx.accounts.stake_record.staked_at = current_time;
        },
    }


    Ok(())

}
