use anchor_lang::prelude::*;
use anchor_spl::{
    token::{ Mint, TokenAccount, Token, Transfer, transfer, MintTo, mint_to },
    associated_token::AssociatedToken,
};

use crate::{ state::{ Staker, Collection, ProgramConfig, StakeRecord, RewardType, NftRecord, Subscription }, StakeError, utils::{calc_reward, calc_tx_fee}, constants::FEES_WALLET, STAKING_ENDS };

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Account<'info, ProgramConfig>,

    #[account(mut)]
    pub staker: Account<'info, Staker>,

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
    pub collection: Account<'info, Collection>,

    #[account(
        mut, 
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"stake-record",
        ],
        bump = stake_record.bump,
        has_one = owner @ StakeError::Unauthorized
    )]
    pub stake_record: Account<'info, StakeRecord>,

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
    pub nft_record: Option<Account<'info, NftRecord>>,

    #[account(mut, address = FEES_WALLET)]
    pub fees_wallet: SystemAccount<'info>,

    #[account(
        mut,
        address = collection.reward_token.unwrap() @ StakeError::InvalidRewardToken
    )]
    pub reward_mint: Option<Box<Account<'info, Mint>>>,

    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = token_authority
    )]
    pub stake_token_vault: Option<Box<Account<'info, TokenAccount>>>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = reward_mint,
        associated_token::authority = owner
    )]
    pub reward_receive_account: Option<Account<'info, TokenAccount>>,

    /// CHECK: this account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority"
        ],
        bump = staker.token_auth_bump
    )]
    pub token_authority: Option<UncheckedAccount<'info>>,

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
            authority: self.token_authority.as_ref().expect("token_authority expected").to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn mint_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.reward_mint.as_ref().expect("reward_mint expected").to_account_info(),
            to: self.reward_receive_account.as_ref().expect("reward_receive_account expected").to_account_info(),
            authority: self.token_authority.as_ref().expect("token_authority expected").to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn claim_handler(ctx: Context<Claim>) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;

    let Staker {
        is_active: staking_status,
        token_auth_bump,
        ..
    } = **staker;

    let Collection {
        minimum_period,
        staking_ends_at,
        is_active: collection_status,
        reward_type,
        ..
    } = **collection;

    let reward_record = &collection.reward;
    let reward_change_time_record = &collection.reward_change_time;
    let staker_key = staker.key();

    let staked_at = ctx.accounts.stake_record.staked_at;

    require_eq!(staking_status, true, StakeError::StakeInactive);
    require_eq!(collection_status, true, StakeError::CollectionInactive);
    require_gte!(staking_ends_at.unwrap_or(STAKING_ENDS), staked_at, StakeError::StakeOver);

    let (reward_tokens, current_time, is_eligible_for_reward) = calc_reward(
        staked_at,
        minimum_period,
        reward_record,
        reward_change_time_record,
        staking_ends_at
    ).unwrap();

    if !is_eligible_for_reward {
        return err!(StakeError::MinimumPeriodNotReached);
    }

    if reward_tokens > 0 {
        let authority_seed = &[
            &b"STAKE"[..],
            &staker_key.as_ref(),
            &b"token-authority"[..],
            &[token_auth_bump],
        ];

        match reward_type {
            RewardType::MintToken => {
                mint_to(
                    ctx.accounts
                        .mint_token_ctx()
                        .with_signer(&[&authority_seed[..]]),
                    reward_tokens,
                )?;
            }
            RewardType::TransferToken => {
                msg!("REWARD {}", reward_tokens);
                transfer(
                    ctx.accounts.transfer_token_ctx().with_signer(&[&authority_seed[..]]),
                    reward_tokens
                )?;
                let collection = &mut ctx.accounts.collection;

                collection.decrease_current_balance(staked_at, current_time)?;
            }
            RewardType::Points => {
                let nft_record = &mut ctx.accounts.nft_record.as_ref().expect("nft_record expected").clone();
                nft_record.add_points(reward_tokens)?;
            }
            _ => {},
        }
    }

    let collection = &mut ctx.accounts.collection;
    
    if current_time < collection.staking_ends_at.unwrap_or(STAKING_ENDS) {    
        collection.update_staked_weight(staked_at, false)?;
        collection.update_staked_weight(current_time, true)?;
    }

    // distribution type stakers should not be reset, so as
    // to not lose eligible for reward status if min period.
    match reward_type {
        RewardType::SolDistribution => {},
        RewardType::TokenDistribution => {},
        _ => {
            ctx.accounts.stake_record.staked_at = current_time;
        },
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


    Ok(())

}
