use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    metadata::{
        mpl_token_metadata::{
            instructions::{
                DelegateStandardV1CpiBuilder, DelegateUtilityV1CpiBuilder, LockV1CpiBuilder,
                TransferV1CpiBuilder,
            },
            types::TokenStandard,
        },
        MasterEditionAccount, Metadata, MetadataAccount, TokenRecordAccount,
    },
    token::{Mint, Token, TokenAccount},
};

use crate::{
    constants::FEES_WALLET,
    state::{Collection, NftRecord, ProgramConfig, RewardType, StakeRecord, Staker, Subscription},
    utils::calc_tx_fee,
    StakeError, STAKING_ENDS,
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account()]
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
        init_if_needed,
        payer = signer,
        space = NftRecord::LEN,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            nft_mint.key().as_ref(),
            b"nft-record",
        ],
        bump,
        // has_one = nft_mint
    )]
    pub nft_record: Option<Box<Account<'info, NftRecord>>>,

    #[account(
        init,
        payer = signer,
        space = StakeRecord::LEN,
        seeds = [
            b"STAKE",
            staker.key().as_ref(),
            nft_mint.key().as_ref(),
            b"stake-record",
        ],
        bump
    )]
    pub stake_record: Box<Account<'info, StakeRecord>>,

    #[account(
        seeds = [b"program-config"],
        bump
    )]
    pub program_config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mint::decimals = 0,
        constraint = nft_mint.supply == 1 @ StakeError::TokenNotNFT
    )]
    nft_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = nft_mint,
        associated_token::authority = signer,
        constraint = nft_token.amount == 1 @ StakeError::TokenAccountEmpty
    )]
    nft_token: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [
            b"metadata",
            Metadata::id().as_ref(),
            nft_mint.key().as_ref()
        ],
        seeds::program = Metadata::id(),
        bump,
        constraint = nft_metadata.collection.as_ref().unwrap().verified @ StakeError::CollectionNotVerified,
        constraint = nft_metadata.collection.as_ref().unwrap().key == collection.collection_mint @ StakeError::InvalidCollection
    )]
    nft_metadata: Box<Account<'info, MetadataAccount>>,

    #[account()]
    nft_edition: Box<Account<'info, MasterEditionAccount>>,

    #[account(mut)]
    owner_token_record: Option<Box<Account<'info, TokenRecordAccount>>>,

    /// CHECK: this account is initialized in the CPI call
    #[account(mut)]
    destination_token_record: Option<AccountInfo<'info>>,

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

    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = nft_mint,
        associated_token::authority = nft_authority
    )]
    pub nft_custody: Option<Box<Account<'info, TokenAccount>>>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, address = FEES_WALLET)]
    pub fees_wallet: SystemAccount<'info>,

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

impl<'info> Stake<'info> {
    pub fn transfer_nft(&self) -> Result<()> {
        let metadata_program = &self.metadata_program;
        let token = &self.nft_token.as_ref().to_account_info();
        let token_owner = &&self.signer.to_account_info();
        let destination_token = self.nft_custody.as_ref().unwrap().to_account_info();
        let destination_owner = &self.nft_authority.to_account_info();
        let mint = &self.nft_mint.to_account_info();
        let metadata = &self.nft_metadata.to_account_info();
        let edition = &self.nft_edition.to_account_info();
        let system_program = &self.system_program.to_account_info();
        let sysvar_instructions = &self.sysvar_instructions.to_account_info();
        let spl_token_program = &&self.token_program.to_account_info();
        let spl_ata_program = &self.associated_token_program.to_account_info();
        let auth_rules_program = self.auth_rules_program.as_ref();
        let auth_rules = self.auth_rules.as_ref();
        let token_record = &self
            .owner_token_record
            .as_ref()
            .map(|token_record| token_record.to_account_info());
        let destination_token_record = self.destination_token_record.as_ref();

        let mut cpi_transfer = TransferV1CpiBuilder::new(&metadata_program);

        cpi_transfer
            .token(token)
            .token_owner(token_owner)
            .destination_token(&destination_token)
            .destination_owner(destination_owner)
            .mint(mint)
            .metadata(metadata)
            .edition(Some(edition))
            .authority(token_owner)
            .payer(token_owner)
            .system_program(system_program)
            .sysvar_instructions(sysvar_instructions)
            .spl_token_program(spl_token_program)
            .spl_ata_program(spl_ata_program)
            .authorization_rules_program(auth_rules_program)
            .authorization_rules(auth_rules)
            .token_record(token_record.as_ref())
            .destination_token_record(destination_token_record)
            .amount(1);

        // performs the CPI
        cpi_transfer.invoke()?;
        Ok(())
    }

    pub fn lock_nft(&self) -> Result<()> {
        let metadata_program = &self.metadata_program;
        let staker_key = &self.staker.key();
        let nft_auth_bump = &self.staker.nft_auth_bump;
        let token = &self.nft_token.to_account_info();
        let token_owner = &self.signer.to_account_info();
        let mint = &self.nft_mint.to_account_info();
        let metadata = &self.nft_metadata;
        let metadata_account_info = &metadata.to_account_info();
        let nft_authority = &self.nft_authority.to_account_info();
        let edition = &self.nft_edition.to_account_info();
        let system_program = &self.system_program.to_account_info();
        let sysvar_instructions = &self.sysvar_instructions.to_account_info();
        let spl_token_program: &&AccountInfo<'_> = &&self.token_program.to_account_info();
        let auth_rules_program = self.auth_rules_program.as_ref();
        let auth_rules = self.auth_rules.as_ref();
        let token_record = &self
            .owner_token_record
            .as_ref()
            .map(|token_record| token_record.to_account_info());

        let txn_signer: &[&[u8]; 4] = &[
            &b"STAKE"[..],
            &staker_key.as_ref(),
            &b"nft-authority"[..],
            &[*nft_auth_bump],
        ];

        if matches!(
            metadata.token_standard,
            Some(TokenStandard::ProgrammableNonFungible)
        ) {
            let mut cpi_delegate = DelegateUtilityV1CpiBuilder::new(&metadata_program);
            cpi_delegate
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
                .token_record(token_record.as_ref())
                .amount(1);

            cpi_delegate.invoke()?;
        } else {
            let mut cpi_delegate = DelegateStandardV1CpiBuilder::new(&metadata_program);
            cpi_delegate
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
                .amount(1);

            cpi_delegate.invoke()?;
        };

        let mut cpi_lock = LockV1CpiBuilder::new(&metadata_program);
        cpi_lock
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

        cpi_lock.invoke_signed(&[txn_signer])?;

        Ok(())
    }
}

pub fn stake_handler<'info>(ctx: Context<'_, '_, '_, 'info, Stake<'info>>) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let current_time = Clock::get().unwrap().unix_timestamp;

    let Staker {
        is_active: staker_active,
        ..
    } = **staker.as_ref();

    let Collection {
        custodial,
        current_stakers_count: current_stakers,
        max_stakers_count: max_stakers,
        is_active: collection_is_active,
        ..
    } = **collection.as_ref();

    require_eq!(staker_active, true, StakeError::StakeInactive);
    require_eq!(collection_is_active, true, StakeError::CollectionInactive);
    require_gt!(max_stakers, current_stakers, StakeError::MaxStakersReached);

    require_gte!(
        current_time,
        collection.staking_starts_at,
        StakeError::StakeNotLive
    );

    require_gt!(
        collection.staking_ends_at.unwrap_or(STAKING_ENDS),
        current_time,
        StakeError::StakeOver
    );

    let owner = ctx.accounts.signer.key();
    let nft_record_bump = ctx.bumps.nft_record;
    let stake_record_bump = ctx.bumps.stake_record;
    let nft_mint = &ctx.accounts.nft_mint;

    if custodial {
        ctx.accounts.transfer_nft()?;
    } else {
        ctx.accounts.lock_nft()?;
    }

    let tx_fee = match staker.get_subscription() {
        Subscription::Custom {
            amount: _,
            stake_fee,
            unstake_fee: _,
            claim_fee: _,
        } => stake_fee,
        _ => ctx.accounts.program_config.stake_fee,
    };

    let tx_fee = calc_tx_fee(staker, tx_fee);

    if tx_fee > 0 {
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.signer.key(),
            &ctx.accounts.fees_wallet.key(),
            tx_fee,
        );

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.fees_wallet.to_account_info(),
            ],
        )?;
    }

    let collection = &mut ctx.accounts.collection;

    match collection.reward_type {
        RewardType::Points => {
            let nft_record = &mut ctx.accounts.nft_record.as_mut().unwrap();

            if nft_record.nft_mint.eq(&Pubkey::default()) {
                ****nft_record = NftRecord::init(nft_mint.key(), nft_record_bump);
            }
        }
        RewardType::TransferToken => {}
        _ => {}
    }
    collection.update_staked_weight(current_time, true)?;

    let stake_record = &mut ctx.accounts.stake_record;
    ***stake_record = StakeRecord::init(owner, nft_mint.key(), current_time, stake_record_bump);

    collection.increase_staker_count()?;
    let staker = &mut ctx.accounts.staker;
    staker.increase_staker_count()
}
