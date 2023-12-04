import * as anchor from "@coral-xyz/anchor"
import { PublicKey, Umi, publicKey } from "@metaplex-foundation/umi"
import assert from "assert"
import { createNft } from "./create-nft"
import { umi } from "./umi"
import { Stake } from "../../target/types/stake"

export function assertErrorLogContains(
  err: {
    logs: string[]
  },
  text: string
) {
  assert.ok(err.logs.find((log) => log.includes(text)))
}

export async function expectFail(func: Function, onError: Function) {
  try {
    await func()
    assert.fail("Expected function to throw")
  } catch (err) {
    if (err.code === "ERR_ASSERTION") {
      throw err
    } else {
      onError(err)
    }
  }
}

export const FEES_WALLET = publicKey("2z1kLqnyyZbcxBEYA7AU9wyhyrJ9Pz8BwBkn6KE4SMqw")

export async function mintNfts(collection: PublicKey, num: number, isPnft: boolean, owner?: PublicKey) {
  return await Promise.all(Array.from(new Array(num).keys()).map((async) => createNft(umi, isPnft, collection, owner)))
}

export async function getStakedItemsForUser(program: anchor.Program<Stake>, bytes: string) {
  return program.account.stakeRecord.all([
    {
      memcmp: {
        bytes,
        offset: 8,
      },
    },
  ])
}

export const USDC = umi.eddsa.createKeypairFromSecretKey(
  new Uint8Array([
    75, 99, 227, 15, 51, 157, 70, 233, 126, 205, 115, 69, 81, 90, 236, 202, 249, 228, 169, 111, 104, 175, 193, 110, 42,
    130, 91, 166, 231, 9, 179, 221, 152, 234, 109, 137, 160, 251, 168, 253, 49, 53, 239, 138, 100, 159, 233, 138, 175,
    155, 187, 39, 143, 183, 73, 122, 11, 108, 66, 27, 167, 16, 227, 0,
  ])
)

export function assertErrorCode(err: any, code) {
  assert.equal(err?.error?.errorCode?.code, code, `Expected code ${code}`)
}

export function assertEqualishLamports(num1: bigint, num2: bigint, msg?: string) {
  assert.ok(Math.abs(Number(num1) - Number(num2)) < 100, msg)
}
