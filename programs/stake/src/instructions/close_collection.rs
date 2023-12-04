use anchor_lang::prelude::*;
use anchor_spl::token::{
    mint_to, set_authority, spl_token::instruction::AuthorityType, transfer, Mint, MintTo,
    SetAuthority, Token, TokenAccount, Transfer,
};

use crate::{
    state::{Collection, RewardType, Staker},
    utils::calc_actual_balance,
    StakeError, STAKING_ENDS,
};

#[derive(Accounts)]
pub struct CloseCollection<'info> {
    #[account(
        mut,
        realloc = staker.current_len() - 32,
        realloc::payer = authority,
        realloc::zero = false,
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
        bump = collection.bump,
    )]
    pub collection: Account<'info, Collection>,

    #[account(
        mut,
        address = collection.reward_token.unwrap() @ StakeError::InvalidRewardToken
    )]
    pub token_mint: Option<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = authority
    )]
    pub token_account: Option<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_authority,
    )]
    pub stake_token_vault: Option<Account<'info, TokenAccount>>,

    /// CHECK: This account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority",
        ],
        bump
    )]
    pub token_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

impl<'info> CloseCollection<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .stake_token_vault
                .as_ref()
                .expect("stake_token_vault expected")
                .to_account_info(),
            to: self
                .token_account
                .as_ref()
                .expect("token_account expected")
                .to_account_info(),
            authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn transfer_auth_ctx(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self
                .token_mint
                .as_ref()
                .expect("token_mint expected")
                .to_account_info(),
            current_authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn mint_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self
                .token_mint
                .as_ref()
                .expect("token_mint expected")
                .to_account_info(),
            to: self
                .stake_token_vault
                .as_ref()
                .expect("stake_token_vault expected")
                .to_account_info(),
            authority: self.token_authority.to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }
}

pub fn close_collection_handler(ctx: Context<CloseCollection>) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let current_time = Clock::get().unwrap().unix_timestamp;

    let Staker {
        token_auth_bump, ..
    } = **staker;

    let Collection {
        current_stakers_count,
        staking_ends_at,
        staked_weight,
        is_active: staking_status,
        reward_type,
        collection_mint,
        reward_token,
        ..
    } = **collection;

    let current_reward = *collection.reward.last().unwrap();
    let last_reward_change_time = *collection.reward_change_time.last().unwrap();
    let staker_key = staker.key();

    require_eq!(staking_status, true, StakeError::StakeInactive);

    let token_auth_seed = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"token-authority"[..],
        &[token_auth_bump],
    ];

    let mut linked_collections: Vec<Collection> = vec![];

    if staker.collections.len() > 1
        && match reward_type {
            RewardType::MintToken => true,
            _ => false,
        }
    {
        let collections = &ctx.remaining_accounts;

        require!(
            staker.collections.clone().into_iter().all(|pk| collections
                .into_iter()
                .find(|coll| coll.key().eq(&pk))
                .is_some()),
            StakeError::CollectionsMissing
        );

        linked_collections = collections
            .into_iter()
            .map(|coll| {
                let c = &coll;
                let data = &mut &**c.try_borrow_mut_data().unwrap();
                Collection::try_deserialize(data).expect("Expected successful deserialize")
            })
            .collect();
    }

    match reward_type {
        RewardType::MintToken => {
            let (_current_actual_balance, accrued_reward, _new_staked_weight) =
                calc_actual_balance(
                    current_stakers_count,
                    staked_weight,
                    current_reward,
                    last_reward_change_time,
                    staking_ends_at,
                    current_time,
                    0,
                    Some(current_time),
                )?;

            if accrued_reward > 0 {
                // mint the total amount of owing tokens to vault, then switch type to TransferToken.
                mint_to(
                    ctx.accounts
                        .mint_token_ctx()
                        .with_signer(&[&token_auth_seed[..]]),
                    accrued_reward,
                )?;
                let collection = &mut ctx.accounts.collection;
                // update reward_type so future owing tokens are transferred not minted.
                collection.reward_type = RewardType::TransferToken;
                collection.increase_current_balance(accrued_reward)?;
            }

            let should_revoke = linked_collections
                .into_iter()
                .filter(|c| !c.collection_mint.eq(&collection_mint))
                .find(|coll| match coll.reward_type {
                    RewardType::MintToken => coll
                        .reward_token
                        .expect("expected token")
                        .key()
                        .eq(&reward_token.unwrap_or(Pubkey::default())),
                    _ => false,
                })
                .is_none();

            if should_revoke {
                set_authority(
                    ctx.accounts
                        .transfer_auth_ctx()
                        .with_signer(&[&token_auth_seed[..]]),
                    AuthorityType::MintTokens,
                    Some(ctx.accounts.authority.key()),
                )?;
            }
        }
        RewardType::TransferToken => {
            let (current_actual_balance, _accrued_reward, _new_staked_weight) =
                calc_actual_balance(
                    current_stakers_count,
                    staked_weight,
                    current_reward,
                    last_reward_change_time,
                    staking_ends_at,
                    current_time,
                    collection.current_balance,
                    None,
                )?;
            if current_actual_balance > 0 {
                transfer(
                    ctx.accounts
                        .transfer_token_ctx()
                        .with_signer(&[&token_auth_seed[..]]),
                    current_actual_balance,
                )?;
                let collection = &mut ctx.accounts.collection;
                collection.current_balance = collection
                    .current_balance
                    .checked_sub(current_actual_balance)
                    .ok_or(StakeError::ProgramSubError)?;
            }
        }
        _ => {}
    }

    let collection = &mut ctx.accounts.collection;

    collection.close_collection();

    // Allow stakers to instantly withdraw their NFTs
    collection.minimum_period = 0;

    // If the staking end time is more than the current time then change it to current
    // This is done to avoid accrual of any new stake rewards
    collection.staking_ends_at = if staking_ends_at.unwrap_or(STAKING_ENDS) > current_time {
        Some(current_time)
    } else {
        staking_ends_at
    };

    let collections: Vec<Pubkey> = staker.collections.clone();
    let staker: &mut Account<'_, Staker> = &mut ctx.accounts.staker;

    staker.collections = collections
        .into_iter()
        .filter(|coll| coll.to_bytes() != ctx.accounts.collection.key().to_bytes())
        .collect();

    Ok(())
}
