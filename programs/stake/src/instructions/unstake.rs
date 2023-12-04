use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        mpl_token_metadata::{
            instructions::{
                RevokeStandardV1CpiBuilder, RevokeUtilityV1CpiBuilder, TransferV1CpiBuilder,
                UnlockV1CpiBuilder,
            },
            types::TokenStandard,
        },
        Metadata, MetadataAccount, TokenRecordAccount,
    },
    token::{
        close_account, mint_to, transfer, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer,
    },
};

use crate::{
    constants::FEES_WALLET,
    state::{Collection, NftRecord, ProgramConfig, RewardType, StakeRecord, Staker, Subscription},
    utils::{calc_reward, calc_tx_fee},
    StakeError,
};

#[derive(Accounts)]
pub struct Unstake<'info> {
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
        bump = collection.bump
    )]
    pub collection: Box<Account<'info, Collection>>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            stake_record.nft_mint.as_ref(),
            b"stake-record",
        ],
        bump = stake_record.bump,
        has_one = nft_mint,
        has_one = owner @ StakeError::Unauthorized,
        close = owner
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    #[account(
        mut,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            nft_record.nft_mint.as_ref(),
            b"nft-record",
        ],
        bump = nft_record.bump,
        has_one = nft_mint,
    )]
    pub nft_record: Option<Box<Account<'info, NftRecord>>>,

    #[account(mut)]
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
    pub reward_receive_account: Option<Box<Account<'info, TokenAccount>>>,

    #[account(
        mint::decimals = 0,
        constraint = nft_mint.supply == 1 @ StakeError::TokenNotNFT
    )]
    nft_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = nft_mint,
        associated_token::authority = owner
    )]
    nft_token: Box<Account<'info, TokenAccount>>,

    /// CHECK: this account is constrained to a specific address
    #[account(mut, address = FEES_WALLET)]
    pub fees_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [
            b"metadata",
            Metadata::id().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = Metadata::id(),
        bump,
    )]
    nft_metadata: Box<Account<'info, MetadataAccount>>,

    /// CHECK: this account is initialized in the CPI call
    #[account(mut)]
    token_record: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    custody_token_record: Option<Box<Account<'info, TokenRecordAccount>>>,

    /// CHECK: checked in CPI call
    master_edition: UncheckedAccount<'info>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = nft_authority,
        constraint = nft_custody.amount == 1 @ StakeError::TokenAccountEmpty,
        close = owner
    )]
    pub nft_custody: Option<Box<Account<'info, TokenAccount>>>,

    /// CHECK: this account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"token-authority",
        ],
        bump = staker.token_auth_bump
    )]
    pub token_authority: Option<UncheckedAccount<'info>>,

    /// CHECK: this account is not read or written
    #[account(
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            b"nft-authority",
        ],
        bump = staker.nft_auth_bump
    )]
    pub nft_authority: UncheckedAccount<'info>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
    /// CHECK: account checked in CPI
    pub sysvar_instructions: AccountInfo<'info>,
    /// CHECK: account checked in CPI
    pub auth_rules: Option<AccountInfo<'info>>,
    /// CHECK: account checked in CPI
    pub auth_rules_program: Option<AccountInfo<'info>>,
}

impl<'info> Unstake<'info> {
    pub fn transfer_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .stake_token_vault
                .as_ref()
                .expect("stake_token_vault missing")
                .to_account_info(),
            to: self
                .reward_receive_account
                .as_ref()
                .expect("reward_receive_account missing")
                .to_account_info(),
            authority: self
                .token_authority
                .as_ref()
                .expect("token_authority expected")
                .to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn mint_token_ctx(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self
                .reward_mint
                .as_ref()
                .expect("reward_mint expected")
                .to_account_info(),
            to: self
                .reward_receive_account
                .as_ref()
                .expect("reward_receive_account expected")
                .to_account_info(),
            authority: self
                .token_authority
                .as_ref()
                .expect("token_authority expected")
                .to_account_info(),
        };

        let cpi_program = self.token_program.to_account_info();

        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn close_account_ctx(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.nft_custody.as_ref().unwrap().to_account_info(),
            destination: self.owner.to_account_info(),
            authority: self.nft_authority.to_account_info(),
        };
        let cpi_program = self.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }

    pub fn transfer_nft(&self) -> Result<()> {
        let metadata_program = &self.metadata_program;
        let staker_key = &self.staker.key();
        let nft_auth_bump = &self.staker.nft_auth_bump;
        let token = &self.nft_custody.as_ref().unwrap().to_account_info();
        let token_owner = &self.nft_authority.to_account_info();
        let destination_token = &self.nft_token.to_account_info();
        let destination_owner = &self.owner.to_account_info();
        let mint = &self.nft_mint.to_account_info();
        let metadata = &self.nft_metadata.to_account_info();
        let edition = &self.master_edition.to_account_info();
        let system_program = &self.system_program.to_account_info();
        let sysvar_instructions = &self.sysvar_instructions.to_account_info();
        let spl_token_program = &&self.token_program.to_account_info();
        let spl_ata_program = &self.associated_token_program.to_account_info();
        let auth_rules_program = self.auth_rules_program.as_ref();
        let auth_rules = self.auth_rules.as_ref();
        let token_record = &self
            .custody_token_record
            .as_ref()
            .map(|token_record| token_record.to_account_info());
        let destination_token_record = &self
            .token_record
            .as_ref()
            .map(|token_record| token_record.to_account_info());

        let mut cpi_transfer = TransferV1CpiBuilder::new(&metadata_program);

        cpi_transfer
            .token(token)
            .token_owner(token_owner)
            .destination_token(destination_token)
            .destination_owner(destination_owner)
            .mint(mint)
            .metadata(metadata)
            .edition(Some(edition))
            .authority(token_owner)
            .payer(destination_owner)
            .system_program(system_program)
            .sysvar_instructions(sysvar_instructions)
            .spl_token_program(spl_token_program)
            .spl_ata_program(spl_ata_program)
            .authorization_rules_program(auth_rules_program)
            .authorization_rules(auth_rules)
            .token_record(token_record.as_ref())
            .destination_token_record(destination_token_record.as_ref())
            .amount(1);

        let txn_signer: &[&[u8]; 4] = &[
            &b"STAKE"[..],
            &staker_key.as_ref(),
            &b"nft-authority"[..],
            &[*nft_auth_bump],
        ];

        // performs the CPI
        cpi_transfer.invoke_signed(&[txn_signer])?;
        Ok(())
    }

    pub fn unlock_nft(&self) -> Result<()> {
        let metadata_program = &self.metadata_program;
        let staker_key = &self.staker.key();
        let nft_auth_bump = &self.staker.nft_auth_bump;
        let token = &self.nft_token.to_account_info();
        let token_owner = &self.owner.to_account_info();
        let mint = &self.nft_mint.to_account_info();
        let metadata = &self.nft_metadata;
        let metadata_account_info = &metadata.to_account_info();
        let nft_authority = &self.nft_authority.to_account_info();
        let edition = &self.master_edition.to_account_info();
        let system_program = &self.system_program.to_account_info();
        let sysvar_instructions = &self.sysvar_instructions.to_account_info();
        let spl_token_program: &&AccountInfo<'_> = &&self.token_program.to_account_info();
        let auth_rules_program = self.auth_rules_program.as_ref();
        let auth_rules = self.auth_rules.as_ref();
        let token_record = &self
            .token_record
            .as_ref()
            .map(|token_record| token_record.to_account_info());

        let txn_signer: &[&[u8]; 4] = &[
            &b"STAKE"[..],
            &staker_key.as_ref(),
            &b"nft-authority"[..],
            &[*nft_auth_bump],
        ];

        let mut cpi_unlock = UnlockV1CpiBuilder::new(&metadata_program);
        cpi_unlock
            .token(token)
            .token_owner(Some(token_owner))
            .mint(mint)
            .metadata(metadata_account_info)
            .edition(Some(edition))
            .authority(nft_authority)
            .payer(token_owner)
            .system_program(system_program)
            .sysvar_instructions(sysvar_instructions)
            .spl_token_program(Some(spl_token_program))
            .authorization_rules_program(auth_rules_program)
            .authorization_rules(auth_rules)
            .token_record(token_record.as_ref());

        cpi_unlock.invoke_signed(&[txn_signer])?;

        if matches!(
            metadata.token_standard,
            Some(TokenStandard::ProgrammableNonFungible)
        ) {
            let mut cpi_revoke = RevokeUtilityV1CpiBuilder::new(&metadata_program);
            cpi_revoke
                .delegate(nft_authority)
                .token(token)
                .mint(mint)
                .metadata(metadata_account_info)
                .master_edition(Some(edition))
                .authority(token_owner)
                .payer(token_owner)
                .system_program(system_program)
                .sysvar_instructions(sysvar_instructions)
                .spl_token_program(Some(spl_token_program))
                .authorization_rules_program(auth_rules_program)
                .authorization_rules(auth_rules)
                .token_record(token_record.as_ref());

            cpi_revoke.invoke()?;
        } else {
            let mut cpi_revoke = RevokeStandardV1CpiBuilder::new(&metadata_program);
            cpi_revoke
                .delegate(nft_authority)
                .token(token)
                .mint(mint)
                .metadata(metadata_account_info)
                .master_edition(Some(edition))
                .authority(token_owner)
                .payer(token_owner)
                .system_program(system_program)
                .sysvar_instructions(sysvar_instructions)
                .spl_token_program(Some(spl_token_program))
                .authorization_rules_program(auth_rules_program)
                .authorization_rules(auth_rules)
                .token_record(token_record.as_ref());

            cpi_revoke.invoke()?;
        };

        Ok(())
    }
}

pub fn unstake_handler(ctx: Context<Unstake>) -> Result<()> {
    let staker = &ctx.accounts.staker;

    // check unchecked master edition account is as metatdata program account
    require_eq!(
        ctx.accounts.master_edition.to_account_info().owner.key(),
        ctx.accounts.metadata_program.key()
    );

    let Staker {
        token_auth_bump,
        nft_auth_bump,
        ..
    } = **staker.as_ref();

    let Collection {
        minimum_period,
        custodial,
        staking_ends_at,
        reward_type,
        lock_for_minimum_period,
        ..
    } = **ctx.accounts.collection;

    let reward = &ctx.accounts.collection.reward;
    let reward_change_time = &ctx.accounts.collection.reward_change_time;

    let staker_key = staker.key();

    let staked_at = ctx.accounts.stake_record.staked_at;

    let (reward_tokens, current_time, is_eligible_for_reward) = calc_reward(
        staked_at,
        minimum_period,
        &reward,
        reward_change_time,
        staking_ends_at,
    )
    .unwrap();

    if lock_for_minimum_period && !is_eligible_for_reward {
        return err!(StakeError::MinimumPeriodNotReached);
    }

    let token_auth_seed = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"token-authority"[..],
        &[token_auth_bump],
    ];

    if is_eligible_for_reward {
        match reward_type {
            RewardType::MintToken => {
                mint_to(
                    ctx.accounts
                        .mint_token_ctx()
                        .with_signer(&[&token_auth_seed[..]]),
                    reward_tokens,
                )?;
            }
            RewardType::TransferToken => {
                transfer(
                    ctx.accounts
                        .transfer_token_ctx()
                        .with_signer(&[&token_auth_seed[..]]),
                    reward_tokens,
                )?;
                let collection = &mut ctx.accounts.collection;
                collection.decrease_current_balance(staked_at, current_time)?;
            }
            RewardType::Points => {
                let nft_record = &mut ctx
                    .accounts
                    .nft_record
                    .as_mut()
                    .expect("nft_record expected");

                msg!("REWARD {}", reward_tokens);

                nft_record.add_points(reward_tokens)?;
            }
            _ => {}
        }
    }

    let txn_signer: &[&[u8]; 4] = &[
        &b"STAKE"[..],
        &staker_key.as_ref(),
        &b"nft-authority"[..],
        &[nft_auth_bump],
    ];

    if custodial {
        ctx.accounts.transfer_nft()?;
        close_account(
            ctx.accounts
                .close_account_ctx()
                .with_signer(&[&txn_signer[..]]),
        )?;
    } else {
        ctx.accounts.unlock_nft()?;
    }

    let tx_fee = match staker.get_subscription() {
        Subscription::Custom {
            amount: _,
            stake_fee: _,
            unstake_fee,
            claim_fee: _,
        } => unstake_fee,
        _ => ctx.accounts.program_config.unstake_fee,
    };

    let tx_fee = calc_tx_fee(&staker, tx_fee);

    if tx_fee > 0 {
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.owner.key(),
            &ctx.accounts.fees_wallet.key(),
            tx_fee,
        );

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.owner.to_account_info(),
                ctx.accounts.fees_wallet.to_account_info(),
            ],
        )?;
    }

    let collection = &mut ctx.accounts.collection;

    collection.update_staked_weight(staked_at, false)?;
    collection.decrease_staker_count()?;
    let staker = &mut ctx.accounts.staker;
    staker.decrease_staker_count()
}
