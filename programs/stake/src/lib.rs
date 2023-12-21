use anchor_lang::prelude::*;

mod instructions;
mod state;
mod utils;

use anchor_spl::token::Mint;

use instructions::*;
use state::Subscription;

declare_id!("STAKEQkGBjkhCXabzB5cUbWgSSvbVJFEm2oEnyWzdKE");

#[cfg(feature = "local-testing")]
pub mod constants {
    use solana_program::{pubkey, pubkey::Pubkey};
    pub const FEES_WALLET: Pubkey = pubkey!("2z1kLqnyyZbcxBEYA7AU9wyhyrJ9Pz8BwBkn6KE4SMqw");
    pub const USDC_MINT_PUBKEY: Pubkey = pubkey!("BHvJMjTHNZpwwbeDTHbuVK7YU8QU7m72jdyQcKFCmKAX");
    pub const SUBSCRIPTION_WALLET: Pubkey = pubkey!("2z1kLqnyyZbcxBEYA7AU9wyhyrJ9Pz8BwBkn6KE4SMqw");
}

#[cfg(not(feature = "local-testing"))]
pub mod constants {
    use solana_program::{pubkey, pubkey::Pubkey};
    pub const FEES_WALLET: Pubkey = pubkey!("2NkHMEEKymjrjjd9DSEprVV4E7nBr6aHzwFeusHxL2Q6");
    pub const USDC_MINT_PUBKEY: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    pub const SUBSCRIPTION_WALLET: Pubkey = pubkey!("XLsdeHxUL83PkfwaP6uvVnhFvqQJSX7K7cBaLy3Guot");
}

#[constant]
pub const STAKING_ENDS: i64 = 2015762363;

#[constant]
pub const WEIGHT: u128 = 1_000_000_000;

#[derive(Accounts)]
pub struct Test<'info> {
    #[account()]
    pub signer: Signer<'info>,
}

use crate::state::FontStyles;
use crate::state::RewardType;

#[program]
pub mod stake {

    use instructions::close_emission::CloseEmission;

    use super::*;

    pub fn init(
        ctx: Context<Init>,
        slug: String,
        name: String,
        remove_branding: bool,
        own_domain: bool,
        subscription: Option<Subscription>,
        start_date: i64,
    ) -> Result<()> {
        init_handler(
            ctx,
            slug,
            name,
            remove_branding,
            own_domain,
            subscription,
            start_date,
        )
    }

    pub fn toggle_stake_active(ctx: Context<ToggleStakeActive>, is_active: bool) -> Result<()> {
        toggle_stake_active_handler(ctx, is_active)
    }

    pub fn init_distribution(
        ctx: Context<InitDistribution>,
        label: String,
        uri: String,
        num_shares: u32,
        amount: u64,
    ) -> Result<()> {
        init_distribution_handler(ctx, label, uri, num_shares, amount)
    }

    pub fn init_collection(
        ctx: Context<InitCollection>,
        custodial: bool,
        token_vault: bool,
        staking_starts_at: Option<i64>,
        max_stakers_count: u64,
    ) -> Result<()> {
        init_collection_handler(ctx, custodial, staking_starts_at, max_stakers_count)
    }

    pub fn delegate_stake(ctx: Context<DelegateStake>) -> Result<()> {
        handle_delegate_stake(ctx)
    }

    pub fn add_emission(
        ctx: Context<AddEmission>,
        reward_type: RewardType,
        reward: Option<u64>,
        start_time: Option<i64>,
        duration: Option<i64>,
        minimum_period: Option<i64>,
        starting_balance: Option<u64>,
    ) -> Result<()> {
        add_emission_handler(
            ctx,
            reward_type,
            reward,
            start_time,
            duration,
            minimum_period,
            starting_balance,
        )
    }

    pub fn distribute(ctx: Context<Distribute>, amount: u64) -> Result<()> {
        distribute_handler(ctx, amount)
    }

    pub fn close_emission<'info>(
        ctx: Context<'_, '_, 'info, 'info, CloseEmission<'info>>,
    ) -> Result<()> {
        close_emission_handler(ctx)
    }

    pub fn update_theme(
        ctx: Context<UpdateTheme>,
        logo: Option<String>,
        background: Option<String>,
        body_font: Option<FontStyles>,
        header_font: Option<FontStyles>,
        primary_color: Option<String>,
        secondary_color: Option<String>,
        dark_mode: Option<bool>,
    ) -> Result<()> {
        update_theme_handler(
            ctx,
            logo,
            background,
            body_font,
            header_font,
            primary_color,
            secondary_color,
            dark_mode,
        )
    }

    pub fn toggle_collection_active(
        ctx: Context<ToggleCollectionActive>,
        active: bool,
    ) -> Result<()> {
        toggle_collection_active_handler(ctx, active)
    }

    pub fn close_collection(ctx: Context<CloseCollection>) -> Result<()> {
        close_collection_handler(ctx)
    }

    pub fn pay_subscription(ctx: Context<PaySubscription>) -> Result<()> {
        pay_subscription_handler(ctx)
    }

    pub fn stake<'info>(
        ctx: Context<'_, '_, 'info, 'info, Stake<'info>>,
        selection: Option<u64>,
    ) -> Result<()> {
        stake_handler(ctx, selection)
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        claim_handler(ctx)
    }

    pub fn unstake<'info>(ctx: Context<'_, '_, 'info, 'info, Unstake<'info>>) -> Result<()> {
        unstake_handler(ctx)
    }

    pub fn force_unstake<'info>(
        ctx: Context<'_, '_, 'info, 'info, ForceUnstake<'info>>,
    ) -> Result<()> {
        force_unstake_handler(ctx)
    }

    pub fn extend_emission(ctx: Context<ExtendEmission>, new_ending_time: i64) -> Result<()> {
        extend_emission_handler(ctx, new_ending_time)
    }

    pub fn add_funds(ctx: Context<AddFunds>, amount: u64) -> Result<()> {
        add_funds_handler(ctx, amount)
    }

    pub fn remove_funds(ctx: Context<RemoveFunds>) -> Result<()> {
        remove_funds_handler(ctx)
    }

    pub fn change_reward(ctx: Context<ChangeReward>, new_reward: u64) -> Result<()> {
        change_reward_handler(ctx, new_reward)
    }

    pub fn close(ctx: Context<Close>) -> Result<()> {
        close_handler(ctx)
    }

    pub fn update_stake_subscription(
        ctx: Context<UpdateStake>,
        subscription: Subscription,
    ) -> Result<()> {
        update_stake_subscription_handler(ctx, subscription)
    }

    pub fn update_stake_remove_branding(
        ctx: Context<UpdateStake>,
        remove_branding: bool,
    ) -> Result<()> {
        update_stake_remove_branding_handler(ctx, remove_branding)
    }

    pub fn update_stake_own_domain(ctx: Context<UpdateStake>, own_domain: String) -> Result<()> {
        update_stake_own_domain_handler(ctx, own_domain)
    }

    /// these are admin only functions
    pub fn update_stake_next_payment_time(
        ctx: Context<UpdateStakeAdmin>,
        next_payment_time: i64,
    ) -> Result<()> {
        update_stake_next_payment_time_handler(ctx, next_payment_time)
    }

    pub fn clear_clugs(ctx: Context<UpdateProgramConfig>) -> Result<()> {
        clear_slugs_handler(ctx)
    }

    pub fn resize(ctx: Context<Resize>) -> Result<()> {
        resize_handler(ctx)
    }

    pub fn add_token(ctx: Context<AddToken>, token_vault: bool) -> Result<()> {
        add_token_handler(ctx, token_vault)
    }

    pub fn init_program_config(
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
        init_program_config_handler(
            ctx,
            stake_fee,
            unstake_fee,
            claim_fee,
            advanced_subscription_fee,
            pro_subscription_fee,
            ultimate_subscription_fee,
            extra_collection_fee,
            remove_branding_fee,
            own_domain_fee,
        )
    }

    pub fn update_program_config(
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
        update_program_config_handler(
            ctx,
            stake_fee,
            unstake_fee,
            claim_fee,
            advanced_subscription_fee,
            pro_subscription_fee,
            ultimate_subscription_fee,
            extra_collection_fee,
            remove_branding_fee,
            own_domain_fee,
        )
    }
}

#[derive(Accounts)]
pub struct VerifyMetadata<'info> {
    pub mint: Account<'info, Mint>,
    /// CHECK: checked in instruction
    pub metadata_account: AccountInfo<'info>,
}

#[error_code]
pub enum StakeError {
    #[msg("Slug must be max 50 chars")]
    SlugTooLong,
    #[msg("Slug must be provided")]
    SlugRequired,
    #[msg("Name must be max 50 chars")]
    NameTooLong,
    #[msg("Name must be provided")]
    NameRequired,
    #[msg("Profanity cannot be used in a name")]
    ProfanityDetected,
    #[msg("Slug already exists - contact us if you think this is an error")]
    SlugExists,
    #[msg("insuficient balance for new staking duration, add funds before extending")]
    InsufficientBalanceInVault,
    #[msg("this STAKE is completed")]
    StakeOver,
    #[msg("this STAKE is not yet live")]
    StakeNotLive,
    #[msg("max stakers have been reached")]
    MaxStakersReached,
    #[msg("this staker is inactive")]
    StakeInactive,
    #[msg("this collection is inactive")]
    CollectionInactive,
    #[msg("this collection is still inactive")]
    CollectionActive,
    #[msg("nft is not included in the allowed collection")]
    InvalidCollection,
    #[msg("collection must be verified")]
    CollectionNotVerified,
    #[msg("token is not an NFT")]
    TokenNotNFT,
    #[msg("no reward mint has been configured for this stake")]
    NoRewardMint,
    #[msg("unexpected reward token")]
    InvalidRewardToken,
    #[msg("unexpected number of remaining accounts")]
    UnexpectedRemainingAccounts,
    #[msg("token account must contain 1 token")]
    TokenAccountEmpty,
    #[msg("the minimum staking period in seconds can't be negative")]
    NegativePeriodValue,
    #[msg("stake ends time must be greater than the current time and the start time")]
    InvalidStakeEndTime,
    #[msg("Stake end time required if using a token vault")]
    StakeEndTimeRequired,
    #[msg("start time cannot be in the past")]
    StartTimeInPast,
    #[msg("max stakers can't be higher than the total collection size")]
    TooManyStakers,
    #[msg("max stakers must be larger than 0")]
    NotEnoughStakers,
    #[msg("failed to convert the time to i64")]
    FailedTimeConversion,
    #[msg("unable to get stake details bump")]
    StakeBumpError,
    #[msg("unable to subtract the given values")]
    ProgramSubError,
    #[msg("unable to multiply the given values")]
    ProgramMulError,
    #[msg("unable to divide the given values")]
    ProgramDivError,
    #[msg("unable to add the given values")]
    ProgramAddError,
    #[msg("minimum staking period not reached")]
    MinimumPeriodNotReached,
    #[msg("failed to build instruction")]
    InstructionBuilderFailed,
    #[msg("payment isn't yet due")]
    PaymentNotDueYet,
    #[msg("no payment due")]
    NoPaymentDue,
    #[msg("only the system admin can use this instruction")]
    AdminOnly,
    #[msg("the current signer doesn't have permission to perform this action")]
    Unauthorized,
    #[msg("this STAKE has reached its maximum collections")]
    MaxCollections,
    #[msg("update authority approval is required for minimum-term locking")]
    UpdateAuthRequired,
    #[msg("enforced locking period cannot be longer than 1 year")]
    LockingPeriodTooLong,
    #[msg("enforced locking period must be longer than 1 second")]
    LockingPeriodTooShort,
    #[msg("an invalid programData account was provided")]
    InvalidProgramData,
    #[msg("addons cannot be added to a STAKE in arrears")]
    StakeInArrears,
    #[msg("duration is required if using a token vault")]
    DurationRequired,
    #[msg("duration must me more than 0")]
    DurationTooShort,
    #[msg("cannot extend as no end date set")]
    CannotExtendNoEndDate,
    #[msg("all linked collections must be passed in remaining accounts")]
    CollectionsMissing,
    #[msg("all emissions must be passed in remaining accounts")]
    EmissionsMissing,
    #[msg("There are no tokens to claim for this collection")]
    NoTokensToClaim,
    #[msg("There are still active stakers who have yet to claim")]
    CollectionHasStakers,
    #[msg("Slug must be a valid URL slug")]
    InvalidSlug,
    #[msg("Only accepts full arweave images")]
    InvalidImage,
    #[msg("Max 63 chars")]
    ImageTooLong,
    #[msg("Only hexadecimal colors are accepted - eg 0BFFD0")]
    InvalidColor,
    #[msg("Cannot close a staker that still has collections")]
    StillHasCollections,
    #[msg("Cannot close a staker that still has staked items")]
    StillHasStakedItems,
    #[msg("Tokens can only be front loaded with enforced minimum period emissions")]
    FrontLoadNotLocked,
    #[msg("Selected emission(s) do not exist")]
    InvalidEmissions,
    #[msg("One or more selected emissions is not active")]
    InvalidEmissionPeriods,
    #[msg("At least one emission must be provided")]
    NoEmissionsToAdd,
    #[msg("Minimum period cannot by used with multiple option emissions")]
    NoMinPeriodWithOption,
    #[msg("Reward required with this emission type")]
    RewardRequired,
    #[msg("Invalid emission")]
    InvalidEmission,
    #[msg("A selection is required for this emission")]
    EmissionSelectionRequired,
    #[msg("This index is invalid")]
    InvalidIndex,
    #[msg("Only one selection type emission can exist per collection")]
    SelectionEmissionExists,
    #[msg("This collection still has active emissions")]
    StillHasEmissions,
    #[msg("The program doesn't have mint auth for this token")]
    NoAuthority,
    #[msg("This emission is not active")]
    EmissionNotActive,
    #[msg("This emission type needs a token mint")]
    TokenMintRequired,
    #[msg("This collection already has a token")]
    TokenExists,
    #[msg("Token vault address is required")]
    TokenVaultRequired,
    #[msg("Invalid creator for NFT")]
    InvalidCreator,
    #[msg("Label max length is 20 chars")]
    LabelTooLong,
    #[msg("Amount must be greater than 0")]
    AmountTooLow,
    #[msg("The total shares have already been funded for this distribution")]
    TotalSharesFunded,
}
