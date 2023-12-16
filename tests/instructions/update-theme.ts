import { Keypair } from "@metaplex-foundation/umi"
import { init, updateTheme } from "../helpers/instructions"
import { umi } from "../helpers/umi"
import { createNewUser, programPaidBy } from "../helper"
import { Program } from "@coral-xyz/anchor"
import { Stake } from "../../target/types/stake"
import { assert } from "chai"
import { isEqual } from "lodash"
import { assertErrorCode, expectFail } from "../helpers/utils"

describe("Updating the theme", () => {
  const slug = "update_theme"
  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey
  let creator: Keypair
  let creatorProgram: Program<Stake>

  const logo1 = "https://arweave.net/_0OVh6PyNNrScO_XQUiT9okWgVnUvYYHIc1YuPciDuI"
  const logo2 = "https://arweave.net/PyrpaoWicsmvsP3Vl-bw7yioFZj8267lDnG7ojJcz7s"
  const bg1 = "https://arweave.net/o6z8aW4LoQZrz-dMWrAXrk-6hW4keWMP53uqJmSyhkI"
  const bg2 = "https://arweave.net/rG0CZJiJKhsuTnChEs9DjeLg8GExeYSh2X2_6_8z_qQ"

  before(async () => {
    creator = await createNewUser()
    creatorProgram = programPaidBy(creator)
    await init(creatorProgram, keypair, slug)
  })

  it("is initialized with a default theme", async () => {
    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.ok(staker.theme, "Expected staker theme")
    assert.equal(staker.theme.logo, null, "Expected logo to not be set")
    assert.ok(isEqual(staker.theme.logos, []), "Expected empty array of logos")
    assert.ok(
      isEqual(staker.theme.backgrounds, ["/bg.png", "/bg2.png", "/bg3.png", "/bg4.png"]),
      "Expected default bgs"
    )
    assert.equal(staker.theme.background, 0, "Expected default bg to be selected")
    assert.ok(
      isEqual(staker.theme.bodyFont.fontFamily, { sourceSans3: {} }),
      "Expected sourceSans3 as default body font"
    )
    assert.ok(
      isEqual(staker.theme.headerFont.fontFamily, { sourceSans3: {} }),
      "Expected sourceSans3 as default header font"
    )
    assert.equal(staker.theme.headerFont.bold, true, "Expected headers to be bold")
    assert.equal(staker.theme.headerFont.uppercase, true, "Expected headers to be uppercase")
    assert.equal(staker.theme.bodyFont.bold, false, "Expected body to be regular")
    assert.equal(staker.theme.bodyFont.uppercase, false, "Expected body to be default")

    assert.equal(staker.theme.primaryColor, "0BFFD0", "default primary color")
    assert.equal(staker.theme.secondaryColor, "0BFFD0", "default primary color")
    assert.equal(staker.theme.darkMode, true, "Expected dark mode")
  })

  it("Can add a new logo", async () => {
    await updateTheme(creatorProgram, stakerId, { logo: logo1 })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.logo, 0)
    assert.equal(staker.theme.logos.length, 1)
    assert.ok(staker.theme.logos.includes(logo1))
  })

  it("Can add another new logo", async () => {
    await updateTheme(creatorProgram, stakerId, { logo: logo2 })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.logo, 1)
    assert.equal(staker.theme.logos.length, 2)
    assert.ok(staker.theme.logos.includes(logo2))
  })

  it("Can select a previously added logo", async () => {
    await updateTheme(creatorProgram, stakerId, { logo: logo1 })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.logo, 0)
    assert.equal(staker.theme.logos.length, 2)
  })

  it("Can update to one of the stock bgs", async () => {
    await updateTheme(creatorProgram, stakerId, { bg: "/bg2.png" })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.background, 1)
    assert.equal(staker.theme.backgrounds.length, 4)
  })

  it("Can update to another one of the stock bgs", async () => {
    await updateTheme(creatorProgram, stakerId, { bg: "/bg3.png" })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.background, 2)
    assert.equal(staker.theme.backgrounds.length, 4)
  })

  it("Can add a new bg", async () => {
    await updateTheme(creatorProgram, stakerId, { bg: bg1 })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.background, 4)
    assert.equal(staker.theme.backgrounds.length, 5)
    assert.ok(staker.theme.backgrounds.includes(bg1))
  })

  it("Cannot add an image not from arweave", async () => {
    await expectFail(
      () => updateTheme(creatorProgram, stakerId, { bg: "https://bobs-images.com/12345" }),
      (err) => assertErrorCode(err, "InvalidImage")
    )
  })

  it("Cannot add an image over 63 chars", async () => {
    await expectFail(
      () =>
        updateTheme(creatorProgram, stakerId, {
          bg: "https://arweave.net/this_is_a_really_long_image_slug_much_too_long_to_be_accepted",
        }),
      (err) => assertErrorCode(err, "ImageTooLong")
    )
  })

  it("cannot add an invalid color", async () => {
    await expectFail(
      () => updateTheme(creatorProgram, stakerId, { primaryColor: "123" }),
      (err) => assertErrorCode(err, "InvalidColor")
    )
    await expectFail(
      () => updateTheme(creatorProgram, stakerId, { secondaryColor: "12345q" }),
      (err) => assertErrorCode(err, "InvalidColor")
    )
  })

  it("can add a valid hex value", async () => {
    await updateTheme(creatorProgram, stakerId, { secondaryColor: "abcdef", primaryColor: "123456" })
    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.equal(staker.theme.primaryColor, "123456", "Expected color to persist")
    assert.equal(staker.theme.secondaryColor, "abcdef", "Expected color to persist")
  })

  it("can add an expected font", async () => {
    await updateTheme(creatorProgram, stakerId, {
      bodyFont: {
        fontFamily: { openSans: {} },
        bold: true,
        uppercase: false,
      },
    })

    const staker = await creatorProgram.account.staker.fetch(stakerId)
    assert.ok(staker.theme.bodyFont.fontFamily.openSans, "Expected body font to be open sans")
    assert.equal(staker.theme.bodyFont.bold, true, "Expected font weight to be bold")
    assert.equal(staker.theme.bodyFont.uppercase, false, "Expected body font to be lowercase")
  })

  it("cannot add an unexpected font", async () => {
    await expectFail(
      () =>
        updateTheme(creatorProgram, stakerId, {
          bodyFont: {
            // @ts-ignore
            fontFamily: { bobsFont: {} },
            bold: true,
            uppercase: false,
          },
        }),
      (err) => assert.ok(err.message.includes("unable to infer src variant"))
    )
  })
})
