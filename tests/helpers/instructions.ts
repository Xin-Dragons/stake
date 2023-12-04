import { PROGRAM_ID as RULES_PROGRAM_ID } from "@metaplex-foundation/mpl-token-auth-rules"
import * as anchor from "@coral-xyz/anchor"
import {
  Keypair,
  PublicKey,
  publicKey,
  sol,
  tokenAmount,
  unwrapOption,
  unwrapOptionRecursively,
} from "@metaplex-foundation/umi"
import {
  DigitalAsset,
  MPL_TOKEN_METADATA_PROGRAM_ID,
  TokenStandard,
  fetchDigitalAsset,
} from "@metaplex-foundation/mpl-token-metadata"
import { umi } from "./umi"
import {
  findNftAuthorityPda,
  findNftMasterEditionPda,
  findNftMetadataPda,
  findNftRecordPda,
  findProgramConfigPda,
  findProgramDataAddress,
  findStakeRecordPda,
  findStakooorCollectionId,
  findTokenAuthorityPda,
  getTokenAccount,
  getTokenRecordPda,
} from "./pdas"
import { fromWeb3JsPublicKey, toWeb3JsKeypair } from "@metaplex-foundation/umi-web3js-adapters"

import { assert } from "chai"
import { BN } from "bn.js"
import { FEES_WALLET, USDC } from "./utils"

import { isEqual } from "lodash"
import { adminProgram } from "../helper"
import { Stake } from "../../target/types/stake"

export const sleep = async (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

export async function stake(program: anchor.Program<Stake>, staker: PublicKey, nft: DigitalAsset) {
  const authRules = unwrapOptionRecursively(nft.metadata.programmableConfig)?.ruleSet ?? null
  const ownerTokenRecord =
    unwrapOption(nft.metadata.tokenStandard) === TokenStandard.ProgrammableNonFungible
      ? getTokenRecordPda(nft.publicKey, fromWeb3JsPublicKey(program.provider.publicKey))
      : null
  const nftAuthority = findNftAuthorityPda(staker)
  const destinationTokenRecord =
    unwrapOption(nft.metadata.tokenStandard) === TokenStandard.ProgrammableNonFungible
      ? getTokenRecordPda(nft.publicKey, nftAuthority)
      : null

  const collection = findStakooorCollectionId(staker, unwrapOption(nft.metadata.collection).key)
  let collectionAccount = await program.account.collection.fetchNullable(collection)

  const nftRecord =
    collectionAccount && isEqual(collectionAccount.rewardType, { points: {} })
      ? findNftRecordPda(staker, nft.publicKey)
      : null

  const nftToken = getTokenAccount(nft.publicKey, fromWeb3JsPublicKey(program.provider.publicKey))
  const nftMetadata = findNftMetadataPda(nft.publicKey)
  const nftEdition = findNftMasterEditionPda(nft.publicKey)
  const programConfig = findProgramConfigPda()

  const nftCustody = getTokenAccount(nft.publicKey, nftAuthority)

  const stakeRecord = findStakeRecordPda(staker, nft.publicKey)

  return await program.methods
    .stake()
    .accounts({
      staker,
      collection,
      nftRecord,
      nftMint: nft.publicKey,
      nftToken,
      nftMetadata,
      nftEdition,
      nftAuthority,
      nftCustody,
      ownerTokenRecord,
      destinationTokenRecord,
      authRules,
      programConfig,
      stakeRecord,
      feesWallet: FEES_WALLET,
      authRulesProgram: RULES_PROGRAM_ID,
      sysvarInstructions: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      metadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
    })
    .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 350_000 })])
    .rpc()
}

export async function unstake(program: anchor.Program<Stake>, staker: PublicKey, nft: DigitalAsset) {
  const collection = findStakooorCollectionId(staker, unwrapOption(nft.metadata.collection).key)
  const collectionAccount = await program.account.collection.fetch(collection)
  const isPnft = unwrapOption(nft.metadata.tokenStandard) === TokenStandard.ProgrammableNonFungible
  const nftRecord = isEqual(collectionAccount.rewardType, { points: {} })
    ? findNftRecordPda(staker, nft.publicKey)
    : null
  const tokenMint = collectionAccount.rewardToken ? fromWeb3JsPublicKey(collectionAccount.rewardToken) : null
  const tokenAuthority = findTokenAuthorityPda(staker)
  const stakeTokenVault = tokenMint ? getTokenAccount(tokenMint, tokenAuthority) : null
  const rewardReceiveAccount = tokenMint
    ? getTokenAccount(tokenMint, fromWeb3JsPublicKey(program.provider.publicKey))
    : null
  const nftAuthority = findNftAuthorityPda(staker)
  const nftCustody = collectionAccount.custodial ? getTokenAccount(nft.publicKey, nftAuthority) : null
  const nftMetadata = nft.metadata.publicKey
  const nftToken = getTokenAccount(nft.publicKey, fromWeb3JsPublicKey(program.provider.publicKey))
  const masterEdition = findNftMasterEditionPda(nft.publicKey)

  const tokenRecord = isPnft ? getTokenRecordPda(nft.publicKey, fromWeb3JsPublicKey(program.provider.publicKey)) : null
  const custodyTokenRecord =
    isPnft && collectionAccount.custodial ? getTokenRecordPda(nft.publicKey, nftAuthority) : null
  const authRules = unwrapOptionRecursively(nft.metadata.programmableConfig)?.ruleSet || null

  const stakeRecord = findStakeRecordPda(staker, nft.publicKey)

  const programConfig = findProgramConfigPda()

  return await program.methods
    .unstake()
    .accounts({
      staker,
      collection,
      nftRecord,
      stakeRecord,
      rewardMint: tokenMint,
      stakeTokenVault,
      rewardReceiveAccount,
      nftMint: nft.publicKey,
      tokenAuthority,
      nftAuthority,
      tokenRecord,
      nftCustody,
      nftMetadata,
      nftToken,
      custodyTokenRecord,
      masterEdition,
      authRules,
      programConfig,
      feesWallet: FEES_WALLET,
      authRulesProgram: RULES_PROGRAM_ID,
      sysvarInstructions: publicKey(anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY),
      metadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
    })
    .preInstructions([anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 350_000 })])
    .rpc()
}

export async function close(program: anchor.Program<Stake>, staker: PublicKey) {
  await program.methods
    .close()
    .accounts({
      staker,
    })
    .rpc()

  const stakerAccount = await program.account.staker.fetch(staker)

  assert.equal(stakerAccount.isActive, false, "Expected staker to be inactive")
}

type RewardType =
  | { transferToken: {} }
  | { mintToken: {} }
  | { points: {} }
  | { solDistribution: {} }
  | { tokenDistribution: {} }
  | { none: {} }

export async function initCollection(
  program: anchor.Program<Stake>,
  staker: PublicKey,
  collectionMintPk: PublicKey,
  custodial: boolean = false,
  tokenMint: PublicKey | null,
  rewardType: RewardType = { transferToken: {} },
  reward: number = 1,
  minimumPeriod: number = 0,
  startTime: anchor.BN | null = null,
  duration: number | null = null,
  lockForMinimumPeriod: boolean = false
) {
  let stakerAccount = await program.account.staker.fetch(staker)
  const collectionMint = await fetchDigitalAsset(umi, collectionMintPk)
  const maxStakerCount = unwrapOption(collectionMint.metadata.collectionDetails).size
  const rewardBn = new BN(reward)
  const minimumPeriodBn = new BN(minimumPeriod)

  const collection = findStakooorCollectionId(staker, collectionMintPk)
  const tokenAuthority = findTokenAuthorityPda(staker)
  const stakeTokenVault = tokenMint ? getTokenAccount(tokenMint, tokenAuthority) : null
  const tokenAccount = tokenMint ? getTokenAccount(tokenMint, fromWeb3JsPublicKey(program.provider.publicKey)) : null

  const usdc = USDC.publicKey

  const sig = await program.methods
    .initCollection(
      custodial,
      rewardType,
      rewardBn,
      minimumPeriodBn,
      startTime,
      duration ? new BN(duration) : null,
      new anchor.BN(Number(maxStakerCount)),
      lockForMinimumPeriod
    )
    .accounts({
      programConfig: findProgramConfigPda(),
      staker,
      collection,
      collectionMint: collectionMintPk,
      tokenMint,
      tokenAuthority,
      tokenAccount,
      stakeTokenVault,
      collectionMetadata: collectionMint.metadata.publicKey,
      usdc,
      usdcAccount: getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)),
      subscriptionWallet: FEES_WALLET,
      subscriptionUsdcAccount: getTokenAccount(usdc, FEES_WALLET),
    })
    .rpc()

  stakerAccount = await program.account.staker.fetch(staker)

  assert.ok(stakerAccount.collections.find((c) => c.toBase58() === collection))

  return sig
}

export type Subscription =
  | { free: {} }
  | { advanced: {} }
  | { pro: {} }
  | { ultimate: {} }
  | { penalty: {} }
  | {
      custom: {
        amount: anchor.BN
        stakeFee: anchor.BN
        unstakeFee: anchor.BN
        claimFee: anchor.BN
      }
    }

export async function init(
  program: anchor.Program<Stake>,
  keypair: Keypair,
  slug: string,
  name: string = "A name",
  subscription: Subscription = { free: {} },
  removeBranding = false,
  ownDomain = false
) {
  const staker = keypair.publicKey
  const tokenAuthority = findTokenAuthorityPda(staker)
  const nftAuthority = findNftAuthorityPda(staker)

  const usdc = USDC.publicKey

  await program.methods
    .init(slug, name, removeBranding, ownDomain, subscription, new BN(0))
    .accounts({
      programConfig: findProgramConfigPda(),
      staker,
      tokenAuthority,
      nftAuthority,
      usdc,
      usdcAccount: getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)),
      subscriptionWallet: FEES_WALLET,
      subscriptionUsdcAccount: getTokenAccount(usdc, FEES_WALLET),
    })
    .signers([toWeb3JsKeypair(keypair)])
    .rpc()

  const stakerAccount = await program.account.staker.fetch(staker)

  assert.ok(stakerAccount.isActive, "Expected staker to be active")

  return stakerAccount
}

export async function claim(program: anchor.Program<Stake>, staker: PublicKey, nft: DigitalAsset) {
  const collectionMintPk = unwrapOption(nft.metadata.collection).key
  const collection = findStakooorCollectionId(staker, collectionMintPk)
  const collectionAccount = await program.account.collection.fetch(collection)
  const nftRecord = isEqual(collectionAccount.rewardType, { points: {} })
    ? findNftRecordPda(staker, nft.publicKey)
    : null
  const rewardMint = collectionAccount.rewardToken ? fromWeb3JsPublicKey(collectionAccount.rewardToken) : null
  const rewardReceiveAccount = rewardMint
    ? getTokenAccount(rewardMint, fromWeb3JsPublicKey(program.provider.publicKey))
    : null
  const tokenAuthority = findTokenAuthorityPda(staker)
  const stakeTokenVault = rewardMint ? getTokenAccount(rewardMint, tokenAuthority) : null
  const programConfig = findProgramConfigPda()

  const stakeRecord = findStakeRecordPda(staker, nft.publicKey)

  return await program.methods
    .claim()
    .accounts({
      staker,
      collection,
      nftRecord,
      rewardReceiveAccount,
      tokenAuthority,
      stakeTokenVault,
      rewardMint,
      programConfig,
      stakeRecord,
      owner: program.provider.publicKey,
      feesWallet: FEES_WALLET,
    })
    .rpc()
}

export async function paySubscription(program: anchor.Program<Stake>, staker: PublicKey) {
  const usdc = USDC.publicKey
  await program.methods
    .paySubscription()
    .accounts({
      programConfig: findProgramConfigPda(),
      staker,
      usdc,
      usdcAccount: getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)),
      subscriptionWallet: FEES_WALLET,
      subscriptionUsdcAccount: getTokenAccount(usdc, FEES_WALLET),
    })
    .rpc()
}

export async function updateStakeNextPaymentTime(program: anchor.Program<Stake>, staker: PublicKey, adjust?: number) {
  const slot = await program.provider.connection.getSlot()
  let slotTime = new BN(await program.provider.connection.getBlockTime(slot))
  if (adjust) {
    if (adjust > 0) {
      slotTime = slotTime.add(new BN(adjust))
    } else {
      slotTime = slotTime.sub(new BN(Math.abs(adjust)))
    }
  }
  await program.methods
    .updateStakeNextPaymentTime(slotTime)
    .accounts({ staker, program: program.programId, programData: findProgramDataAddress() })
    .rpc()
  return slotTime
}

export async function updateSubscription(
  program: anchor.Program<Stake>,
  staker: PublicKey,
  subscription: Subscription
) {
  const isAdmin = program.provider.publicKey.equals(adminProgram.provider.publicKey)
  const usdc = isAdmin ? null : USDC.publicKey
  const usdcAccount = usdc ? getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)) : null
  const subscriptionUsdcAccount = usdc ? getTokenAccount(usdc, FEES_WALLET) : null
  const programId = isAdmin ? program.programId : null
  const programData = program ? findProgramDataAddress() : null
  const subscriptionWallet = usdc ? FEES_WALLET : null

  return await program.methods
    .updateStakeSubscription(subscription)
    .accounts({
      staker,
      programConfig: findProgramConfigPda(),
      usdc,
      usdcAccount,
      subscriptionUsdcAccount,
      program: programId,
      programData,
      subscriptionWallet,
    })
    .rpc()
}

export async function updateOwnDomain(program: anchor.Program<Stake>, staker: PublicKey, ownDomain: boolean) {
  const isAdmin = program.provider.publicKey.equals(adminProgram.provider.publicKey)
  const usdc = isAdmin ? null : USDC.publicKey
  const usdcAccount = usdc ? getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)) : null
  const subscriptionUsdcAccount = usdc ? getTokenAccount(usdc, FEES_WALLET) : null
  const programId = isAdmin ? program.programId : null
  const programData = program ? findProgramDataAddress() : null
  const subscriptionWallet = usdc ? FEES_WALLET : null
  return await program.methods
    .updateStakeOwnDomain(ownDomain)
    .accounts({
      programConfig: findProgramConfigPda(),
      staker,
      program: programId,
      programData,
      usdc,
      usdcAccount,
      subscriptionUsdcAccount,
      subscriptionWallet,
    })
    .rpc()
}

export async function updateRemoveBranding(program: anchor.Program<Stake>, staker: PublicKey, removeBranding: boolean) {
  const isAdmin = program.provider.publicKey.equals(adminProgram.provider.publicKey)
  const usdc = isAdmin ? null : USDC.publicKey
  const usdcAccount = usdc ? getTokenAccount(usdc, fromWeb3JsPublicKey(program.provider.publicKey)) : null
  const subscriptionUsdcAccount = usdc ? getTokenAccount(usdc, FEES_WALLET) : null
  const programId = isAdmin ? program.programId : null
  const programData = program ? findProgramDataAddress() : null
  const subscriptionWallet = usdc ? FEES_WALLET : null
  return await program.methods
    .updateStakeRemoveBranding(removeBranding)
    .accounts({
      programConfig: findProgramConfigPda(),
      staker,
      program: programId,
      programData,
      usdc,
      usdcAccount,
      subscriptionUsdcAccount,
      subscriptionWallet,
    })
    .rpc()
}

export async function closeCollection(
  program: anchor.Program<Stake>,
  staker: PublicKey,
  collectionMintPk: PublicKey,
  remainingAccounts: anchor.web3.PublicKey[] = []
) {
  const collection = findStakooorCollectionId(staker, collectionMintPk)
  const collectionAccount = await program.account.collection.fetch(collection)
  const tokenMint = collectionAccount.rewardToken ? fromWeb3JsPublicKey(collectionAccount.rewardToken) : null
  const tokenAuthority = findTokenAuthorityPda(staker)
  return await program.methods
    .closeCollection()
    .accounts({
      staker,
      collection,
      tokenMint,
      tokenAccount: tokenMint ? getTokenAccount(tokenMint, fromWeb3JsPublicKey(program.provider.publicKey)) : null,
      tokenAuthority,
      stakeTokenVault: tokenMint ? getTokenAccount(tokenMint, tokenAuthority) : null,
    })
    .remainingAccounts(
      remainingAccounts.map((pubkey) => {
        return {
          pubkey,
          isWritable: false,
          isSigner: false,
        }
      })
    )
    .rpc()
}

export async function initProgramConfig(program: anchor.Program<Stake>) {
  const programConfig = findProgramConfigPda()

  const usdc = USDC.publicKey

  await program.methods
    .initProgramConfig(
      new BN(Number(sol(0.008).basisPoints)), // STAKE
      new BN(Number(sol(0.01).basisPoints)), // UNSTAKE
      new BN(Number(sol(0.004).basisPoints)), // CLAIM
      new BN(Number(tokenAmount(175, "USDC", 9).basisPoints)), // ADVANCED FEE
      new BN(Number(tokenAmount(300, "USDC", 9).basisPoints)), // PRO FEE
      new BN(Number(tokenAmount(500, "USDC", 9).basisPoints)), // ULTIMATE FEE
      new BN(Number(tokenAmount(75, "USDC", 9).basisPoints)), // REMOVE BRANDING
      new BN(Number(tokenAmount(50, "USDC", 9).basisPoints)), // OWN DOMAIN
      new BN(Number(tokenAmount(50, "USDC", 9).basisPoints)) // EXTRA COLLECTION
    )
    .accounts({
      programConfig,
      programData: findProgramDataAddress(),
      program: program.programId,
      usdc,
      subscriptionWallet: FEES_WALLET,
      subscriptionUsdcAccount: getTokenAccount(usdc, FEES_WALLET),
    })
    .rpc()
}

export async function changeReward(
  program: anchor.Program<Stake>,
  reward: number,
  staker: PublicKey,
  collectionMintPk: PublicKey
) {
  await program.methods
    .changeReward(new BN(reward))
    .accounts({
      staker,
      collection: findStakooorCollectionId(staker, collectionMintPk),
    })
    .rpc()
}

export async function addFunds(
  program: anchor.Program<Stake>,
  amount: anchor.BN,
  staker: PublicKey,
  collectionMintPk: PublicKey
) {
  const collection = findStakooorCollectionId(staker, collectionMintPk)
  const collectionAccount = await program.account.collection.fetch(collection)
  const tokenAuth = findTokenAuthorityPda(staker)
  await program.methods
    .addFunds(amount)
    .accounts({
      staker,
      collection,
      rewardMint: collectionAccount.rewardToken,
      tokenAccount: getTokenAccount(
        fromWeb3JsPublicKey(collectionAccount.rewardToken),
        fromWeb3JsPublicKey(program.provider.publicKey)
      ),
      stakeTokenVault: getTokenAccount(fromWeb3JsPublicKey(collectionAccount.rewardToken), tokenAuth),
      tokenAuthority: tokenAuth,
    })
    .rpc()
}

export async function updateProgramConfig(
  program: anchor.Program<Stake>,
  stakeFee: anchor.BN | null = null,
  unstakeFee: anchor.BN | null = null,
  claimFee: anchor.BN | null = null,
  advancedSubscriptionFee: anchor.BN | null = null,
  proSubscriptionFee: anchor.BN | null = null,
  ultimateSubscriptionFee: anchor.BN | null = null,
  extraCollectionFee: anchor.BN | null = null,
  removeBrandingFee: anchor.BN | null = null,
  ownDomainFee: anchor.BN | null = null
) {
  return await program.methods
    .updateProgramConfig(
      stakeFee,
      unstakeFee,
      claimFee,
      advancedSubscriptionFee,
      proSubscriptionFee,
      ultimateSubscriptionFee,
      extraCollectionFee,
      removeBrandingFee,
      ownDomainFee
    )
    .accounts({
      programConfig: findProgramConfigPda(),
      program: program.programId,
      programData: findProgramDataAddress(),
    })
    .rpc()
}

export async function removeFunds(program: anchor.Program<Stake>, staker: PublicKey, collectionMintPk: PublicKey) {
  const tokenAuthority = findTokenAuthorityPda(staker)
  const collection = findStakooorCollectionId(staker, collectionMintPk)
  const stakerCollectionAccount = await program.account.collection.fetch(collection)
  const tokenMint = fromWeb3JsPublicKey(stakerCollectionAccount.rewardToken)

  await program.methods
    .removeFunds()
    .accounts({
      staker,
      collection,
      tokenAuthority,
      stakeTokenVault: getTokenAccount(tokenMint, tokenAuthority),
      tokenAccount: getTokenAccount(tokenMint, fromWeb3JsPublicKey(program.provider.publicKey)),
      rewardMint: tokenMint,
    })
    .rpc()
}

type FontFamily =
  | { roboto: {} }
  | { openSans: {} }
  | { montserrat: {} }
  | { lato: {} }
  | { poppins: {} }
  | { sourceSans3: {} }
  | { leagueGothic: {} }
  | { raleway: {} }
  | { notoSans: {} }
  | { inter: {} }
  | { robotoSlab: {} }
  | { merriweather: {} }
  | { playfairDisplay: {} }
  | { robotoMono: {} }
  | { quattrocento: {} }
  | { quattrocentoSans: {} }
  | { kanit: {} }
  | { nunito: {} }
  | { workSans: {} }

type Font = {
  fontFamily: FontFamily
  uppercase: boolean
  bold: boolean
}

export async function updateTheme(
  program: anchor.Program<Stake>,
  stakerId: PublicKey,
  updates: {
    logo?: string
    bg?: string
    bodyFont?: Font
    headerFont?: Font
    primaryColor?: string
    secondaryColor?: string
    darkMode?: boolean
  }
) {
  return await program.methods
    .updateTheme(
      updates.logo || null,
      updates.bg || null,
      updates.bodyFont || null,
      updates.headerFont || null,
      updates.primaryColor || null,
      updates.secondaryColor || null,
      updates.darkMode || null
    )
    .accounts({ staker: stakerId })
    .rpc()
}
