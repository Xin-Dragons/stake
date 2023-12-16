import * as anchor from "@coral-xyz/anchor"
import { createAssociatedToken, fetchMint, mintTokensTo } from "@metaplex-foundation/mpl-toolbox"
import { FEES_WALLET, USDC, expectFail } from "./helpers/utils"
import { getTokenAccount } from "./helpers/pdas"
import { Keypair, createSignerFromKeypair, sol, tokenAmount, transactionBuilder } from "@metaplex-foundation/umi"
import { umi } from "./helpers/umi"
import { toWeb3JsKeypair } from "@metaplex-foundation/umi-web3js-adapters"
import { initProgramConfig } from "./helpers/instructions"
import { createToken } from "./helpers/create-token"
import { Stake } from "../target/types/stake"

anchor.setProvider(anchor.AnchorProvider.env())
export const adminProgram = anchor.workspace.Stake as anchor.Program<Stake>

before(async () => {
  await createToken(umi, BigInt(1_000), 9, createSignerFromKeypair(umi, USDC))
  await umi.rpc.airdrop(FEES_WALLET, sol(1))
  console.log("HI")
  await initProgramConfig(adminProgram)
  console.log("BYE")
})

export function programPaidBy(payer: Keypair): anchor.Program<Stake> {
  const newProvider = new anchor.AnchorProvider(
    adminProgram.provider.connection,
    new anchor.Wallet(toWeb3JsKeypair(payer)),
    {}
  )

  return new anchor.Program(adminProgram.idl, adminProgram.programId, newProvider)
}

export async function createNewUser() {
  const kp = umi.eddsa.generateKeypair()

  const sig = await umi.rpc.airdrop(kp.publicKey, sol(100))

  const bal = await umi.rpc.getBalance(kp.publicKey)

  const mint = await fetchMint(umi, USDC.publicKey)

  await umi.rpc.airdrop(kp.publicKey, sol(100))
  await transactionBuilder()
    .add(
      createAssociatedToken(umi, {
        mint: USDC.publicKey,
        owner: kp.publicKey,
      })
    )
    .add(
      mintTokensTo(umi, {
        mint: USDC.publicKey,
        token: getTokenAccount(USDC.publicKey, kp.publicKey),
        amount: tokenAmount(10_000, "USDC", 9).basisPoints,
      })
    )
    .sendAndConfirm(umi)
  return kp
}
