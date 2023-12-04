import { Program } from "@coral-xyz/anchor"
import { Keypair, PublicKey, sol, tokenAmount } from "@metaplex-foundation/umi"
import { adminProgram, createNewUser, programPaidBy } from "../helper"
import { findProgramConfigPda, getTokenAccount } from "../helpers/pdas"
import {
  claim,
  init,
  initCollection,
  paySubscription,
  sleep,
  stake,
  unstake,
  updateStakeNextPaymentTime,
  updateSubscription,
} from "../helpers/instructions"
import { FEES_WALLET, USDC, assertErrorCode, expectFail } from "../helpers/utils"
import { fetchToken } from "@metaplex-foundation/mpl-toolbox"
import { umi } from "../helpers/umi"
import { assert } from "chai"
import { isEqual } from "lodash"
import { BN } from "bn.js"
import { DigitalAsset } from "@metaplex-foundation/mpl-token-metadata"
import { createCollection } from "../helpers/create-collection"
import { createNft } from "../helpers/create-nft"
import { Stake } from "../../target/types/stake"

describe("Update stakooor", () => {
  let slug = "update_stakooor"
  let creator: Keypair
  let user: Keypair
  let creatorProgram: Program<Stake>
  let userProgram: Program<Stake>
  let nft: DigitalAsset
  let collection: DigitalAsset
  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  before(async () => {
    creator = await createNewUser()
    creatorProgram = programPaidBy(creator)
    user = await createNewUser()
    userProgram = programPaidBy(user)
    collection = await createCollection(umi)
    nft = await createNft(umi, true, collection.publicKey, user.publicKey)
    await init(creatorProgram, keypair, slug)
    await initCollection(creatorProgram, stakerId, collection.publicKey, false, null, { points: {} })
  })

  it("cannot change the subscription as non-stakooor auth", async () => {
    await expectFail(
      () => updateSubscription(userProgram, stakerId, { pro: {} }),
      (err) => assertErrorCode(err, "Unauthorized")
    )
  })

  it("can change the subscription as stakooor auth, paying the subscription fee", async () => {
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    const stakooorBefore = await creatorProgram.account.staker.fetch(stakerId)
    await updateSubscription(creatorProgram, stakerId, { pro: {} })
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))

    const stakooorAfter = await creatorProgram.account.staker.fetch(stakerId)
    const programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())

    assert.ok(isEqual(stakooorAfter.subscription, { pro: {} }), "Expected subscription to have been updated")
    assert.ok(balanceAfter.amount < balanceBefore.amount, "Expected USDC balance to have reduced")
    assert.equal(
      balanceBefore.amount - balanceAfter.amount,
      BigInt(programConfig.proSubscriptionFee.toString()),
      "Expected to have paid for a full month subscription"
    )

    assert.ok(
      stakooorBefore.nextPaymentTime.eq(stakooorAfter.nextPaymentTime),
      "Expected next payment time to not have changed"
    )
  })

  it("qualifies for the cheaper tx fees right away", async () => {
    const balanceBefore = await umi.rpc.getBalance(FEES_WALLET)
    await stake(userProgram, stakerId, nft)
    const balanceAfter = await umi.rpc.getBalance(FEES_WALLET)

    const programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())

    assert.equal(
      balanceAfter.basisPoints - balanceBefore.basisPoints,
      BigInt(programConfig.stakeFee.toNumber() * 0.2),
      "Expected pro stake rate (20%) to be charged"
    )

    await unstake(userProgram, stakerId, nft)
    const balanceEnd = await umi.rpc.getBalance(FEES_WALLET)

    assert.equal(
      balanceEnd.basisPoints - balanceAfter.basisPoints,
      BigInt(programConfig.unstakeFee.toNumber() * 0.2),
      "Expected pro unstake rate (20%) to be charged"
    )
  })

  it("Cannot set the subscription type to custom as non system admin", async () => {
    await expectFail(
      () =>
        updateSubscription(creatorProgram, stakerId, {
          custom: { amount: new BN(0), claimFee: new BN(0), stakeFee: new BN(0), unstakeFee: new BN(0) },
        }),
      (err) => assertErrorCode(err, "Unauthorized")
    )
  })

  it("Can downgrade the subscription type, and not be charged any amount", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId, 3)

    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    await updateSubscription(creatorProgram, stakerId, { advanced: {} })
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))

    const stakoooor = await creatorProgram.account.staker.fetch(stakerId)
    assert.ok(isEqual(stakoooor.subscription, { advanced: {} }), "Expected subscription to have been updated")
    assert.equal(balanceBefore.amount, balanceAfter.amount, "Expected to not have paid for the downgrade")
    assert.ok(isEqual(stakoooor.prevSubscription, { pro: {} }), "Expected previous subscription to be set")
    assert.ok(
      stakoooor.subscriptionLiveDate.eq(stakoooor.nextPaymentTime),
      "Expected subscription live date to be set to the next payment time"
    )
  })

  it("Doesn't charge the higher tx fee if the downgrade isn't live yet", async () => {
    const balanceBefore = await umi.rpc.getBalance(FEES_WALLET)
    await stake(userProgram, stakerId, nft)
    const balanceAfter = await umi.rpc.getBalance(FEES_WALLET)

    const programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())

    assert.equal(
      balanceAfter.basisPoints - balanceBefore.basisPoints,
      BigInt(programConfig.stakeFee.toNumber() * 0.2),
      "Expected pro stake rate (20%) to be charged"
    )

    await unstake(userProgram, stakerId, nft)
    const balanceEnd = await umi.rpc.getBalance(FEES_WALLET)

    assert.equal(
      balanceEnd.basisPoints - balanceAfter.basisPoints,
      BigInt(programConfig.unstakeFee.toNumber() * 0.2),
      "Expected pro unstake rate (20%) to be charged"
    )
  })

  it("Charges higher tx fee if downgrade is live", async () => {
    await sleep(2000)
    const balanceBefore = await umi.rpc.getBalance(FEES_WALLET)
    await stake(userProgram, stakerId, nft)
    const balanceAfter = await umi.rpc.getBalance(FEES_WALLET)

    const programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())

    assert.equal(
      balanceAfter.basisPoints - balanceBefore.basisPoints,
      BigInt(programConfig.stakeFee.toNumber() * 0.5),
      "Expected advanced stake rate (50%) to be charged"
    )

    await unstake(userProgram, stakerId, nft)
    const balanceEnd = await umi.rpc.getBalance(FEES_WALLET)

    assert.equal(
      balanceEnd.basisPoints - balanceAfter.basisPoints,
      BigInt(programConfig.unstakeFee.toNumber() * 0.5),
      "Expected advanced unstake rate (50%) to be charged"
    )
  })

  it("can unilaterally change a subscription as the system admin", async () => {
    await updateSubscription(adminProgram, stakerId, { ultimate: {} })
    const stakooor = await creatorProgram.account.staker.fetch(stakerId)
    assert.ok(isEqual(stakooor.subscription, { ultimate: {} }), "Expected change to have been made")
  })

  it("can set a custom subscription as the system admin", async () => {
    await updateSubscription(adminProgram, stakerId, {
      custom: {
        amount: new BN(String(tokenAmount(1_000, "USDC", 9).basisPoints)),
        stakeFee: new BN(String(sol(0.123).basisPoints)),
        claimFee: new BN(String(sol(0.456).basisPoints)),
        unstakeFee: new BN(String(sol(0.789).basisPoints)),
      },
    })

    const stakooor = await creatorProgram.account.staker.fetch(stakerId)
    assert.ok(stakooor.subscription.custom, "Expected custom subscription to exist")
  })

  it("is charged the custom fees for stake claim and unstake", async () => {
    const balanceBefore = await umi.rpc.getBalance(FEES_WALLET)
    await stake(userProgram, stakerId, nft)

    const balanceAfterStake = await umi.rpc.getBalance(FEES_WALLET)
    await claim(userProgram, stakerId, nft)

    const balanceAfterClaim = await umi.rpc.getBalance(FEES_WALLET)
    await unstake(userProgram, stakerId, nft)

    const balanceAfterUnstake = await umi.rpc.getBalance(FEES_WALLET)

    assert.equal(
      balanceAfterStake.basisPoints - balanceBefore.basisPoints,
      tokenAmount(0.123, "USDC", 9).basisPoints,
      "Expected custom stake fee to have been charged"
    )

    assert.equal(
      balanceAfterClaim.basisPoints - balanceAfterStake.basisPoints,
      tokenAmount(0.456, "USDC", 9).basisPoints,
      "Expected custom stake fee to have been charged"
    )

    assert.equal(
      balanceAfterUnstake.basisPoints - balanceAfterClaim.basisPoints,
      tokenAmount(0.789, "USDC", 9).basisPoints,
      "Expected custom stake fee to have been charged"
    )
  })

  it("is charged the custom subscription fee when renewing", async () => {
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    await paySubscription(creatorProgram, stakerId)
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))

    assert.equal(
      balanceBefore.amount - balanceAfter.amount,
      tokenAmount(1_000, "USDC", 9).basisPoints,
      "Expected to pay the custom subscription amount"
    )
  })
})
