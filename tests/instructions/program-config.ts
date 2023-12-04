import { sol, tokenAmount } from "@metaplex-foundation/umi"
import { initProgramConfig, updateProgramConfig } from "../helpers/instructions"
import { assertErrorLogContains, expectFail } from "../helpers/utils"
import { BN } from "bn.js"
import { findProgramConfigPda, findProgramDataAddress } from "../helpers/pdas"
import { assert } from "chai"
import { umi } from "../helpers/umi"
import { adminProgram, createNewUser, programPaidBy } from "../helper"

describe("Program config", () => {
  it("Cannot be created after init", async () => {
    await expectFail(
      () => initProgramConfig(adminProgram),
      (err) => assertErrorLogContains(err, "already in use")
    )
  })

  it("Cannot be updated with a non-admin wallet", async () => {
    const newUser = await createNewUser()
    const program = programPaidBy(newUser)
    await expectFail(
      () => updateProgramConfig(program, new BN(Number(sol(0.01).basisPoints))),
      (err) => assert.equal(err.error.errorCode.code, "AdminOnly")
    )
  })

  it("Can update the stake fee", async () => {
    const newFee = new BN(Number(sol(0.01).basisPoints))
    await updateProgramConfig(adminProgram, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.stakeFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the unstake fee", async () => {
    const newFee = new BN(Number(sol(0.01234).basisPoints))
    await updateProgramConfig(adminProgram, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.unstakeFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the claim fee", async () => {
    const newFee = new BN(Number(sol(0.0456).basisPoints))
    await updateProgramConfig(adminProgram, undefined, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.claimFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the advanced subscription fee", async () => {
    const newFee = new BN(Number(tokenAmount(25, "USDC", 9).basisPoints))
    await updateProgramConfig(adminProgram, undefined, undefined, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.advancedSubscriptionFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the pro subscription fee", async () => {
    const newFee = new BN(Number(tokenAmount(200, "USDC", 9).basisPoints))

    await updateProgramConfig(adminProgram, undefined, undefined, undefined, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.proSubscriptionFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the ultimate subscription fee", async () => {
    const newFee = new BN(Number(tokenAmount(350, "USDC", 9).basisPoints))
    await updateProgramConfig(adminProgram, undefined, undefined, undefined, undefined, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.ultimateSubscriptionFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the extra collection fee", async () => {
    const newFee = new BN(Number(tokenAmount(50, "USDC", 9).basisPoints))
    await updateProgramConfig(adminProgram, undefined, undefined, undefined, undefined, undefined, undefined, newFee)

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.extraCollectionFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the remove branding fee", async () => {
    const newFee = new BN(Number(tokenAmount(100, "USDC", 9).basisPoints))
    await updateProgramConfig(
      adminProgram,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      newFee
    )

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.removeBrandingFee.eq(newFee), "Expected the new fee to be applied")
  })

  it("Can update the own domain fee", async () => {
    const newFee = new BN(Number(tokenAmount(75, "USDC", 9).basisPoints))
    await updateProgramConfig(
      adminProgram,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      newFee
    )

    const programConfig = await adminProgram.account.programConfig.fetch(findProgramConfigPda())
    assert.ok(programConfig.ownDomainFee.eq(newFee), "Expected the new fee to be applied")
  })
})
