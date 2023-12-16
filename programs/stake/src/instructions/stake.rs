use std::ops::Deref;

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
use emission::Emission;

use crate::{
    constants::FEES_WALLET,
    state::{
        emission, Collection, NftRecord, ProgramConfig, RewardType, StakeRecord, Staker,
        Subscription,
    },
    utils::calc_tx_fee,
    StakeError, STAKING_ENDS,
};

#[derive(Accounts)]
pub struct Stake<'info> {
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
        constraint = Option::is_none(&nft_metadata.collection) || nft_metadata.collection.as_ref().unwrap().verified && nft_metadata.collection.as_ref().unwrap().key == collection.collection_mint @ StakeError::InvalidCollection,
        constraint = Option::is_some(&nft_metadata.collection) || nft_metadata.creators.as_ref().unwrap().first().unwrap().address == collection.collection_mint && nft_metadata.creators.as_ref().unwrap().first().unwrap().verified @ StakeError::InvalidCreator
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

pub fn stake_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, Stake<'info>>,
    selection: Option<u64>,
) -> Result<()> {
    let staker = &ctx.accounts.staker;
    let collection = &ctx.accounts.collection;
    let current_time = Clock::get().unwrap().unix_timestamp;
    let owner = ctx.accounts.signer.key();
    let nft_mint = &ctx.accounts.nft_mint;
    let nft_record_bump = ctx.bumps.nft_record;
    let stake_record_bump = ctx.bumps.stake_record;

    // require!(
    //     ctx.remaining_accounts
    //         .iter()
    //         .all(|emission| collection.emissions.contains(&emission.key())),
    //     StakeError::InvalidEmissions
    // );

    let mut pending_claim: u64 = 0;
    let mut can_claim_at: i64 = 0;
    let mut has_points: bool = false;

    let mut emissions: Vec<Pubkey> = vec![];

    if Option::is_some(&collection.token_emission) {
        let account = ctx
            .remaining_accounts
            .iter()
            .find(|acc| collection.token_emission.unwrap().to_bytes() == acc.key().to_bytes())
            .unwrap();

        let mut token_emission =
            Account::<'info, Emission>::try_from(account).expect("Expected emission to be passed");

        require_keys_eq!(
            token_emission.key(),
            collection.token_emission.unwrap(),
            StakeError::InvalidEmission
        );

        require!(token_emission.active, StakeError::EmissionNotActive);
        token_emission.update_staked_weight(current_time, true)?;
        token_emission.increase_staked_items()?;

        emissions.push(token_emission.key());

        token_emission.exit(ctx.program_id)?;
    }

    if Option::is_some(&collection.points_emission) {
        let account = ctx
            .remaining_accounts
            .iter()
            .find(|acc| collection.points_emission.unwrap().to_bytes() == acc.key().to_bytes())
            .unwrap();

        let mut points_emission =
            Account::<'info, Emission>::try_from(account).expect("Expected emission to be passed");

        require_keys_eq!(
            points_emission.key(),
            collection.points_emission.unwrap(),
            StakeError::InvalidEmission
        );

        require!(points_emission.active, StakeError::EmissionNotActive);

        // let nft_record = &mut ctx.accounts.nft_record.as_ref().unwrap();

        // if nft_record.nft_mint.eq(&Pubkey::default()) {
        //     ****nft_record = NftRecord::init(nft_mint.key(), nft_record_bump);
        // }

        points_emission.update_staked_weight(current_time, true)?;
        points_emission.increase_staked_items()?;

        emissions.push(points_emission.key());

        points_emission.exit(ctx.program_id)?;
        // let mut emission =
        // Account::<'info, Emission>::try_from(account).expect("Expected emission to be passed");
    }

    if Option::is_some(&collection.selection_emission) {
        let account = ctx
            .remaining_accounts
            .iter()
            .find(|acc| collection.selection_emission.unwrap().to_bytes() == acc.key().to_bytes())
            .unwrap();

        let mut selection_emission =
            Account::<'info, Emission>::try_from(account).expect("Expected emission to be passed");

        require_keys_eq!(
            selection_emission.key(),
            collection.selection_emission.unwrap(),
            StakeError::InvalidEmission
        );

        require!(selection_emission.active, StakeError::EmissionNotActive);

        let options = match selection_emission.reward_type.clone() {
            RewardType::Selection { options } => options,
            _ => vec![],
        };

        require_keys_eq!(
            selection_emission.key(),
            collection.selection_emission.unwrap(),
            StakeError::InvalidEmission
        );

        require_gt!(
            options.len(),
            selection.unwrap() as usize,
            StakeError::InvalidIndex
        );
        let option = options[selection.expect("Expected selection to be defined") as usize];
        require_gte!(
            option.reward,
            selection_emission.current_balance,
            StakeError::InsufficientBalanceInVault
        );

        let balance_owing = option.reward * (option.duration as u64);

        require_gte!(
            selection_emission.current_balance,
            balance_owing,
            StakeError::InsufficientBalanceInVault
        );

        selection_emission.staked_weight = selection_emission
            .staked_weight
            .checked_add(balance_owing.into())
            .ok_or(StakeError::ProgramAddError)?;

        pending_claim = balance_owing;
        can_claim_at = current_time + option.duration;

        selection_emission.increase_staked_items()?;

        emissions.push(selection_emission.key());

        selection_emission.exit(ctx.program_id)?;
    }

    if Option::is_some(&collection.distribution_emission) {
        let account = ctx
            .remaining_accounts
            .iter()
            .find(|acc| {
                collection.distribution_emission.unwrap().to_bytes() == acc.key().to_bytes()
            })
            .unwrap();

        let mut distribution_emission =
            Account::<'info, Emission>::try_from(account).expect("Expected emission to be passed");

        require_keys_eq!(
            distribution_emission.key(),
            collection.distribution_emission.unwrap(),
            StakeError::InvalidEmission
        );

        require!(distribution_emission.active, StakeError::EmissionNotActive);

        distribution_emission.increase_staked_items()?;
        emissions.push(distribution_emission.key());
        distribution_emission.exit(ctx.program_id)?;
    }

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

    if has_points {}

    let stake_record = &mut ctx.accounts.stake_record;
    ***stake_record = StakeRecord::init(
        staker.key(),
        owner,
        nft_mint.key(),
        emissions,
        current_time,
        pending_claim,
        can_claim_at,
        stake_record_bump,
    );

    collection.increase_staker_count()?;
    let staker = &mut ctx.accounts.staker;
    staker.increase_staker_count()
}
