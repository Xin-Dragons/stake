import { assert } from "chai"
import { createToken } from "../helpers/create-token"
import {
  closeCollection,
  init,
  initCollection,
  paySubscription,
  stake,
  updateOwnDomain,
  updateRemoveBranding,
  updateStakeNextPaymentTime,
  updateSubscription,
} from "../helpers/instructions"
import { findProgramConfigPda, getTokenAccount } from "../helpers/pdas"
import { umi } from "../helpers/umi"
import { FEES_WALLET, USDC, assertErrorCode, expectFail, mintNfts } from "../helpers/utils"
import { BN } from "bn.js"
import { toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters"
import { DigitalAsset } from "@metaplex-foundation/mpl-token-metadata"
import { createCollection } from "../helpers/create-collection"
import { Keypair, PublicKey } from "@metaplex-foundation/umi"
import { fetchToken } from "@metaplex-foundation/mpl-toolbox"
import { adminProgram, createNewUser, programPaidBy } from "../helper"
import { Program } from "@coral-xyz/anchor"
import { Stake } from "../../target/types/stake"

describe("pay subscription", () => {
  const slug = "pay_subscription"
  const usdc = USDC.publicKey
  let owner: Keypair
  let user: Keypair
  let ownerProgram: Program<Stake>
  let userProgram: Program<Stake>
  let tokenMint: PublicKey
  let nfts: DigitalAsset[]
  let nfts2: DigitalAsset[]
  let collection: DigitalAsset
  let collection2: DigitalAsset
  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  before(async () => {
    owner = await createNewUser()
    user = await createNewUser()
    ownerProgram = programPaidBy(owner)
    userProgram = programPaidBy(user)
    tokenMint = await createToken(umi, BigInt(10_000_000), 9, undefined, owner.publicKey)
    await init(ownerProgram, keypair, slug)
    collection = await createCollection(umi)
    nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
    collection2 = await createCollection(umi)
    nfts2 = await mintNfts(collection2.publicKey, 10, true)
  })

  it("cannot pay if payment isn't due yet", async () => {
    await expectFail(
      () => paySubscription(ownerProgram, stakerId),
      (err) => {
        assert.equal(err.error.errorCode.code, "PaymentNotDueYet")
      }
    )
  })

  it("cannot update the payment due date if not an admin", async () => {
    await expectFail(
      () => updateStakeNextPaymentTime(ownerProgram, stakerId),
      (err) => assertErrorCode(err, "AdminOnly")
    )
  })

  it("cannot pay if there is nothing owing", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId)

    await expectFail(
      () => paySubscription(ownerProgram, stakerId),
      (err) => assert.equal(err.error.errorCode.code, "NoPaymentDue")
    )
  })

  it("can pay subscription", async () => {
    await updateSubscription(adminProgram, stakerId, { pro: {} })

    const stakerBefore = await ownerProgram.account.staker.fetch(stakerId)
    const balanceBefore = await ownerProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    await paySubscription(ownerProgram, stakerId)

    const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())

    const stakerAfter = await ownerProgram.account.staker.fetch(stakerId)
    const balanceAfter = await ownerProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    assert.ok(
      stakerBefore.nextPaymentTime.add(new BN(60 * 60 * 24 * 30)).eq(stakerAfter.nextPaymentTime),
      "Expected next payment time to be advanced by 30 days"
    )

    assert.ok(
      new BN(balanceBefore.value.amount).add(programConfig.proSubscriptionFee).eq(new BN(balanceAfter.value.amount)),
      "expected pro fee to be charged"
    )
  })

  it("can pay as a third party", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId)
    const usdc = USDC.publicKey

    const stakerBefore = await userProgram.account.staker.fetch(stakerId)
    const balanceBefore = await fetchToken(umi, getTokenAccount(usdc, FEES_WALLET))

    await paySubscription(userProgram, stakerId)

    const programConfig = await userProgram.account.programConfig.fetch(findProgramConfigPda())

    const stakerAfter = await userProgram.account.staker.fetch(stakerId)
    const balanceAfter = await fetchToken(umi, getTokenAccount(usdc, FEES_WALLET))

    assert.ok(
      stakerBefore.nextPaymentTime.add(new BN(60 * 60 * 24 * 30)).eq(stakerAfter.nextPaymentTime),
      "Expected next payment time to be advanced by 30 days"
    )

    assert.equal(
      balanceBefore.amount + BigInt(programConfig.proSubscriptionFee.toString()),
      balanceAfter.amount,
      "expected pro fee to be charged"
    )
  })

  it("can pay subscription with bolt ons", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId)
    await updateSubscription(adminProgram, stakerId, { advanced: {} })

    await updateRemoveBranding(adminProgram, stakerId, true)
    await updateOwnDomain(adminProgram, stakerId, true)

    const stakerBefore = await adminProgram.account.staker.fetch(stakerId)
    const balanceBefore = await adminProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    await paySubscription(ownerProgram, stakerId)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())

    const stakerAfter = await adminProgram.account.staker.fetch(stakerId)
    const balanceAfter = await adminProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    assert.ok(
      stakerBefore.nextPaymentTime.add(new BN(60 * 60 * 24 * 30)).eq(stakerAfter.nextPaymentTime),
      "Expected next payment time to be advanced by 30 days"
    )

    assert.ok(
      new BN(balanceBefore.value.amount)
        .add(programConfig.advancedSubscriptionFee)
        .add(programConfig.ownDomainFee)
        .add(programConfig.removeBrandingFee)
        .eq(new BN(balanceAfter.value.amount)),
      "expected advance fee with branding bolt ons to be charged"
    )
  })

  it("can pay a lapsed subscription", async () => {
    const time = await updateStakeNextPaymentTime(adminProgram, stakerId, -(60 * 60 * 24 * 15))

    const balanceBefore = await adminProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    await paySubscription(ownerProgram, stakerId)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())

    const stakerAfter = await adminProgram.account.staker.fetch(stakerId)
    const balanceAfter = await adminProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    const expectedTimeIsh = time.add(new BN(60 * 60 * 24 * 45))

    assert.ok(
      stakerAfter.nextPaymentTime.gte(expectedTimeIsh) &&
        stakerAfter.nextPaymentTime.lte(expectedTimeIsh.add(new BN(2))),
      "expected next payment time to be advanced by 30 days from now"
    )

    assert.ok(
      new BN(balanceBefore.value.amount)
        .add(programConfig.advancedSubscriptionFee)
        .add(programConfig.ownDomainFee)
        .add(programConfig.removeBrandingFee)
        .eq(new BN(balanceAfter.value.amount)),
      "expected advance fee with branding bolt ons to be charged"
    )
  })

  it("can add 2 collections, increasing monthly cost", async () => {
    await initCollection(
      ownerProgram,
      stakerId,
      collection.publicKey,
      false,
      tokenMint,
      { transferToken: {} },
      1,
      0,
      undefined,
      360
    )
    await initCollection(
      ownerProgram,
      stakerId,
      collection2.publicKey,
      false,
      tokenMint,
      { transferToken: {} },
      1,
      0,
      undefined,
      360
    )

    await updateStakeNextPaymentTime(adminProgram, stakerId)
    const stakerBefore = await ownerProgram.account.staker.fetch(stakerId)
    const balanceBefore = await ownerProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    await paySubscription(ownerProgram, stakerId)

    const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())

    const stakerAfter = await ownerProgram.account.staker.fetch(stakerId)
    const balanceAfter = await ownerProgram.provider.connection.getTokenAccountBalance(
      toWeb3JsPublicKey(getTokenAccount(usdc, FEES_WALLET))
    )

    assert.ok(
      stakerBefore.nextPaymentTime.add(new BN(60 * 60 * 24 * 30)).eq(stakerAfter.nextPaymentTime),
      "Expected next payment time to be advanced by 30 days"
    )

    assert.ok(
      new BN(balanceBefore.value.amount)
        .add(programConfig.advancedSubscriptionFee)
        .add(programConfig.ownDomainFee)
        .add(programConfig.removeBrandingFee)
        .add(programConfig.extraCollectionFee)
        .eq(new BN(balanceAfter.value.amount)),
      "expected advance fee with branding bolt ons to be charged"
    )
  })

  it("normal subscription tx fee is charged if in grace period", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId, -(60 * 60 * 24 * 5))
    const balanceBefore = await ownerProgram.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))
    await stake(userProgram, stakerId, nfts[0])
    const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())
    const balanceAfter = await ownerProgram.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))

    assert.ok(
      new BN(balanceBefore).add(programConfig.stakeFee.div(new BN(2))).eq(new BN(balanceAfter)),
      "Expected stake fee to be discounted 50%"
    )
  })

  it("penalty tx fee is charged if extends grace period with lapsed bolt ons", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId, -(60 * 60 * 24 * 15))
    const balanceBefore = await ownerProgram.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))
    await stake(userProgram, stakerId, nfts[1])
    const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())
    const balanceAfter = await ownerProgram.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))

    assert.ok(
      new BN(balanceBefore).add(programConfig.stakeFee.mul(new BN(2))).eq(new BN(balanceAfter)),
      "Expected to pay penalty stake fee"
    )
  })

  it("basic tx fee is charged if extends grace period without bolt ons", async () => {
    await updateStakeNextPaymentTime(adminProgram, stakerId, -(60 * 60 * 24 * 15))
    await updateOwnDomain(adminProgram, stakerId, false)
    await updateRemoveBranding(adminProgram, stakerId, false)
    await closeCollection(ownerProgram, stakerId, collection2.publicKey)
    const balanceBefore = await umi.rpc.getBalance(FEES_WALLET)
    await stake(userProgram, stakerId, nfts[2])
    const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())
    const balanceAfter = await umi.rpc.getBalance(FEES_WALLET)

    assert.equal(
      balanceAfter.basisPoints - balanceBefore.basisPoints,
      BigInt(programConfig.stakeFee.toString()),

      "Expected to pay basic stake fee"
    )
  })
})
