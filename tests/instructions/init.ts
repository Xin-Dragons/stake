import * as anchor from "@coral-xyz/anchor"
import type { Program } from "@coral-xyz/anchor"
import { umi } from "../helpers/umi"
import { assert } from "chai"
import { findProgramConfigPda, getTokenAccount } from "../helpers/pdas"
import { init, paySubscription } from "../helpers/instructions"
import { fetchToken } from "@metaplex-foundation/mpl-toolbox"
import { Keypair } from "@metaplex-foundation/umi"
import { USDC, assertErrorCode, assertErrorLogContains, expectFail } from "../helpers/utils"
import { createNewUser, programPaidBy } from "../helper"
import { isEqual } from "lodash"
import { Stake } from "../../target/types/stake"

describe.only("Init Stakooor", () => {
  let creator: Keypair
  let creatorProgram: Program<Stake>
  let programConfig: anchor.IdlAccounts<Stake>["programConfig"]
  const stakooor1 = umi.eddsa.generateKeypair()

  before(async () => {
    creator = await createNewUser()
    creatorProgram = programPaidBy(creator)
    programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())
  })

  it("can initialize a new stakooor", async () => {
    const slug = "test_collection"
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))

    await init(creatorProgram, stakooor1, slug, "Test Collection", { free: {} })
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    const staker = await creatorProgram.account.staker.fetch(stakooor1.publicKey)
    assert.equal(staker.slug, slug, "Expected slug to be persisted")
    assert.equal(balanceBefore.amount, balanceAfter.amount, "Expected to have not paid for the subscription")
  })

  it("Cannot include a name over 50 chars", async () => {
    await expectFail(
      () =>
        init(
          creatorProgram,
          umi.eddsa.generateKeypair(),
          "long_name",
          "This is a very very very long name, too long to be accepted by the program"
        ),
      (err) => console.log(err)
    )
  })

  it("Must include a name", async () => {
    await expectFail(
      () => init(creatorProgram, umi.eddsa.generateKeypair(), "missing_name", ""),
      (err) => console.log(err)
    )
  })

  it("Cannot include profanity", async () => {
    it("Must include a name", async () => {
      await expectFail(
        () => init(creatorProgram, umi.eddsa.generateKeypair(), "fuck", "FUCK"),
        (err) => console.log(err)
      )
    })
  })

  it("cannot re initialize an existing stakooor", async () => {
    const slug = "test_collection2"
    await expectFail(
      () => init(creatorProgram, stakooor1, slug),
      (err) => assertErrorLogContains(err, "already in use")
    )
  })

  it("cannot use an existing slug", async () => {
    const slug = "test_collection"
    await expectFail(
      () => init(creatorProgram, umi.eddsa.generateKeypair(), slug),
      (err) => assertErrorCode(err, "SlugExists")
    )
  })

  it("cannot be initialized without a slug", async () => {
    const slug = ""
    await expectFail(
      () => init(creatorProgram, umi.eddsa.generateKeypair(), slug),
      (err) => assertErrorCode(err, "SlugRequired")
    )
  })

  it("cannot use a slug over 50 chars", async () => {
    const slug = "this_is_a_very_very_long_slug_we_want_to_allow_stupid_long_slugs_but_not_ridiculous"
    await expectFail(
      () => init(creatorProgram, umi.eddsa.generateKeypair(), slug),
      (err) => assertErrorCode(err, "SlugTooLong")
    )
  })

  it("cannot use an invalid slug", async () => {
    const slug = "This is not allowed"
    await expectFail(
      () => init(creatorProgram, umi.eddsa.generateKeypair(), slug),
      (err) => assertErrorCode(err, "InvalidSlug")
    )
  })

  it("cannot pay if nothing owing", async () => {
    await expectFail(
      () => paySubscription(creatorProgram, stakooor1.publicKey),
      (err) => assertErrorCode(err, "PaymentNotDueYet")
    )
  })

  it("can initialize a new stakooor on an advanced plan", async () => {
    const slug = "advanced_stakooor"
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    const keypair = umi.eddsa.generateKeypair()
    await init(creatorProgram, keypair, slug, { advanced: {} })
    const stakooor = await creatorProgram.account.staker.fetch(keypair.publicKey)
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    assert.ok(isEqual(stakooor.subscription, { advanced: {} }))
    assert.equal(balanceBefore.amount - balanceAfter.amount, BigInt(programConfig.advancedSubscriptionFee.toString()))
  })

  it("can init a new collection on a pro plan", async () => {
    const slug = "pro_stakooor"
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    const keypair = umi.eddsa.generateKeypair()
    await init(creatorProgram, keypair, slug, { pro: {} })
    const stakooor = await creatorProgram.account.staker.fetch(keypair.publicKey)
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    assert.ok(isEqual(stakooor.subscription, { pro: {} }))
    assert.equal(balanceBefore.amount - balanceAfter.amount, BigInt(programConfig.proSubscriptionFee.toString()))
  })

  it("can init a new collection on a ultimate plan", async () => {
    const slug = "ultimate_stakooor"
    const balanceBefore = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    const keypair = umi.eddsa.generateKeypair()
    await init(creatorProgram, keypair, slug, { ultimate: {} })
    const stakooor = await creatorProgram.account.staker.fetch(keypair.publicKey)
    const balanceAfter = await fetchToken(umi, getTokenAccount(USDC.publicKey, creator.publicKey))
    assert.ok(isEqual(stakooor.subscription, { ultimate: {} }))
    assert.equal(balanceBefore.amount - balanceAfter.amount, BigInt(programConfig.ultimateSubscriptionFee.toString()))
  })
})
