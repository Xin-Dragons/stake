import * as anchor from "@coral-xyz/anchor"
import { Keypair, PublicKey, publicKey, sol, tokenAmount } from "@metaplex-foundation/umi"
import { Stake } from "../../target/types/stake"
import { umi } from "../helpers/umi"
import { createNewUser, programPaidBy } from "../helper"
import {
  findProgramConfigPda,
  findShareRecordPda,
  findStakeRecordPda,
  findStakooorCollectionId,
  findTokenAuthorityPda,
  getTokenAccount,
} from "../helpers/pdas"
import {
  addEmission,
  addToken,
  claim,
  distribute,
  init,
  initCollection,
  initDistribution,
  sleep,
  stake,
  toggleCollection,
  toggleStake,
  unstake,
} from "../helpers/instructions"
import { createCollection } from "../helpers/create-collection"
import {
  DigitalAsset,
  TokenMintToFailedError,
  collect,
  findCollectionAuthorityRecordPda,
} from "@metaplex-foundation/mpl-token-metadata"
import { createToken } from "../helpers/create-token"
import { AuthorityType, fetchToken, safeFetchToken, setAuthority } from "@metaplex-foundation/mpl-toolbox"
import { assertErrorCode, expectFail, mintNfts } from "../helpers/utils"
import { assert } from "chai"
import { BN } from "bn.js"

const BASE_FEE = BigInt(5000)

describe.only("Emissions", () => {
  let creator: Keypair
  let creatorProgram: anchor.Program<Stake>
  let userProgram: anchor.Program<Stake>
  let programConfig: anchor.IdlAccounts<Stake>["programConfig"]
  const staker = umi.eddsa.generateKeypair()
  let user: Keypair
  let stakeAccount: anchor.IdlAccounts<Stake>["staker"]
  let collectionNft: DigitalAsset
  let nfts: DigitalAsset[]
  let token: PublicKey
  let collection: PublicKey

  before(async () => {
    collectionNft = await createCollection(umi)
    collection = findStakooorCollectionId(staker.publicKey, collectionNft.publicKey)
    user = await createNewUser()
    nfts = await mintNfts(collectionNft.publicKey, 10, true, user.publicKey)

    creator = await createNewUser()
    token = await createToken(umi, BigInt(10_000), 0, undefined, creator.publicKey)
    await setAuthority(umi, {
      owned: token,
      owner: umi.identity.publicKey,
      newAuthority: creator.publicKey,
      authorityType: AuthorityType.MintTokens,
    }).sendAndConfirm(umi)
    creatorProgram = programPaidBy(creator)
    userProgram = programPaidBy(user)
    programConfig = await creatorProgram.account.programConfig.fetch(findProgramConfigPda())
    stakeAccount = await init(creatorProgram, staker, "emissions", "Emissions test", token, { free: {} })
    await initCollection(creatorProgram, staker.publicKey, collectionNft.publicKey, false)
    await addToken(creatorProgram, staker.publicKey, token, false)
    await toggleCollection(creatorProgram, staker.publicKey, collection, true)
    await toggleStake(creatorProgram, staker.publicKey, true)
  })

  describe("Standard token mint emission", () => {
    const emission = umi.eddsa.generateKeypair()
    it("can add a single token emission", async () => {
      await addEmission(creatorProgram, emission, staker.publicKey, collection, { token }, 1, 0, null, 3600)
      const collectionAccount = await creatorProgram.account.collection.fetch(collection)

      const emissionAccount = await creatorProgram.account.emission.fetch(emission.publicKey)
      assert.ok(emissionAccount.active, "Expected emission to be active")
      assert.equal(
        collectionAccount.tokenEmission.toBase58(),
        emission.publicKey,
        "Expected emission to be added to collection"
      )
    })

    it("Can stake an NFT", async () => {
      await stake(userProgram, staker.publicKey, nfts[0])
      const stakeRecord = await userProgram.account.stakeRecord.fetch(
        findStakeRecordPda(staker.publicKey, nfts[0].publicKey)
      )
      const emissionAcc = await userProgram.account.emission.fetch(emission.publicKey)

      assert.equal(emissionAcc.stakedItems.toNumber(), 1, "Expected 1 staked item")

      assert.equal(stakeRecord.owner.toBase58(), user.publicKey, "Expected correct user")
    })

    it("Can claim tokens", async () => {
      await sleep(1_000)

      const balanceBefore = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)
      await claim(userProgram, staker.publicKey, nfts[0], emission.publicKey)

      const balanceAfter = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)

      assert.ok(balanceAfter > balanceBefore, "Expected balance to have increased")
    })

    it("can unstake", async () => {
      await sleep(1_000)
      const balanceBefore = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)

      await unstake(userProgram, staker.publicKey, nfts[0])
      const balanceAfter = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)
      assert.ok(balanceAfter > balanceBefore, "Expected balance to have increased")
    })

    it("can close the emission", async () => {
      const collectionPk = findStakooorCollectionId(staker.publicKey, collectionNft.publicKey)
      let collectionAccount = await creatorProgram.account.collection.fetch(collectionPk)
      const tokenAuthority = findTokenAuthorityPda(staker.publicKey)

      await creatorProgram.methods
        .closeEmission()
        .accounts({
          staker: staker.publicKey,
          emission: emission.publicKey,
          tokenMint: token,
          collection: collectionPk,
          tokenAccount: getTokenAccount(token, creator.publicKey),
          tokenAuthority,
          stakeTokenVault: stakeAccount.tokenVault ? getTokenAccount(token, tokenAuthority) : null,
        })
        .rpc()

      collectionAccount = await creatorProgram.account.collection.fetch(collectionPk)
      assert.equal(collectionAccount.tokenEmission, null, "Expected token emission to be null")
    })
  })

  describe("SOL distribution", () => {
    const distribution = umi.eddsa.generateKeypair()
    it.only("can create a new SOL distribution", async () => {
      await initDistribution(
        creatorProgram,
        staker.publicKey,
        collection,
        distribution,
        "Test distribution",
        "http://test.com",
        3,
        new BN(String(sol(100).basisPoints))
      )
    })

    it.only("Can distribute", async () => {
      await stake(userProgram, staker.publicKey, nfts[0])
      const stakeRecord = findStakeRecordPda(staker.publicKey, nfts[0].publicKey)
      const balanceBefore = await umi.rpc.getBalance(creator.publicKey)
      await distribute(
        creatorProgram,
        staker.publicKey,
        distribution.publicKey,
        stakeRecord,
        new BN(String(sol(1).basisPoints))
      )
      const balanceAfter = await umi.rpc.getBalance(creator.publicKey)

      const shareAccount = await creatorProgram.account.shareRecord.fetch(
        findShareRecordPda(distribution.publicKey, nfts[0].publicKey)
      )

      console.log(balanceBefore.basisPoints, balanceAfter.basisPoints)

      assert.equal(
        balanceBefore.basisPoints,
        balanceAfter.basisPoints + sol(1).basisPoints + BigInt(5000),
        "Expected balace to reduce by 1 sol"
      )

      console.log(shareAccount)
    })

    it("Can distribute again", async () => {
      const stakeRecord = findStakeRecordPda(staker.publicKey, nfts[1].publicKey)
      await creatorProgram.methods
        .distribute(new BN(String(sol(5).basisPoints)))
        .accounts({
          staker: staker.publicKey,
          stakeRecord,
          collection,
          distributionEmission: emission.publicKey,
          nftMint: nfts[1].publicKey,
        })
        .rpc()
        .catch((err) => console.log(err))

      const stakeAccount = await creatorProgram.account.stakeRecord.fetch(stakeRecord)
      assert.equal(stakeAccount.solBalance.toString(), String(sol(15).basisPoints))
    })

    it("Can claim sol", async () => {
      const balanceBefore = await umi.rpc.getBalance(user.publicKey)
      await claim(userProgram, staker.publicKey, nfts[1], emission.publicKey)
      const balanceAfter = await umi.rpc.getBalance(user.publicKey)

      assert.equal(
        balanceAfter.basisPoints - balanceBefore.basisPoints,
        BigInt(sol(15).basisPoints) - BASE_FEE - BigInt(programConfig.claimFee.toString()),
        "expected S to be withdrawn"
      )
    })

    it("Can distribute again", async () => {
      const stakeRecord = findStakeRecordPda(staker.publicKey, nfts[1].publicKey)
      await creatorProgram.methods
        .distribute(new BN(String(sol(5).basisPoints)))
        .accounts({
          staker: staker.publicKey,
          stakeRecord,
          collection,
          distributionEmission: emission.publicKey,
          nftMint: nfts[1].publicKey,
        })
        .rpc()

      const stakeAccount = await creatorProgram.account.stakeRecord.fetch(stakeRecord)
      assert.equal(stakeAccount.solBalance.toString(), String(sol(5).basisPoints))
    })

    it("can unstake and claim", async () => {
      const balanceBefore = await umi.rpc.getBalance(user.publicKey)
      const acc = await umi.rpc.getAccount(findStakeRecordPda(staker.publicKey, nfts[1].publicKey))
      await unstake(userProgram, staker.publicKey, nfts[1])
      const balanceAfter = await umi.rpc.getBalance(user.publicKey)

      const len = (acc.exists && acc.data.length) || 0
      const rentAmount = await umi.rpc.getRent(len)

      assert.equal(
        balanceAfter.basisPoints,
        balanceBefore.basisPoints +
          BigInt(sol(5).basisPoints) +
          rentAmount.basisPoints -
          BASE_FEE -
          BigInt(programConfig.unstakeFee.toString()),
        "expected SOL to be withdrawn"
      )
    })

    it("can close the emission", async () => {
      const collectionPk = findStakooorCollectionId(staker.publicKey, collectionNft.publicKey)
      let collectionAccount = await creatorProgram.account.collection.fetch(collectionPk)
      const tokenAuthority = findTokenAuthorityPda(staker.publicKey)

      await creatorProgram.methods
        .closeEmission()
        .accounts({
          staker: staker.publicKey,
          emission: emission.publicKey,
          tokenMint: token,
          collection: collectionPk,
          tokenAccount: getTokenAccount(token, creator.publicKey),
          tokenAuthority,
          stakeTokenVault: collectionAccount.tokenVault ? getTokenAccount(token, tokenAuthority) : null,
        })
        .rpc()

      collectionAccount = await creatorProgram.account.collection.fetch(collectionPk)
      assert.equal(collectionAccount.distributionEmission, null, "Expected distribution emission to be null")
    })

    it("can no longer distribute sol to any staked NFTs", async () => {
      await stake(userProgram, staker.publicKey, nfts[2])
      await expectFail(
        () =>
          creatorProgram.methods
            .distribute(new BN(String(sol(5).basisPoints)))
            .accounts({
              staker: staker.publicKey,
              stakeRecord: findStakeRecordPda(staker.publicKey, nfts[2].publicKey),
              collection,
              distributionEmission: emission.publicKey,
              nftMint: nfts[2].publicKey,
            })
            .rpc(),
        (err) => assertErrorCode(err, "InvalidEmission")
      )
    })
  })

  describe("multiple emissions", () => {
    const tokenEmission = umi.eddsa.generateKeypair()
    const distributionEmission = umi.eddsa.generateKeypair()
    it("can add a single distribution emission", async () => {
      const collectionPk = findStakooorCollectionId(staker.publicKey, collectionNft.publicKey)
      await addEmission(creatorProgram, tokenEmission, staker.publicKey, collection, { token }, 1, 0, null, 3600)
      await addEmission(creatorProgram, distributionEmission, staker.publicKey, collectionPk, { distribution: {} })

      const tokenEmissionAccount = await creatorProgram.account.emission.fetch(tokenEmission.publicKey)
      assert.ok(tokenEmissionAccount.active, "Expected emission to be active")
      const distributionEmissionAccount = await creatorProgram.account.emission.fetch(distributionEmission.publicKey)
      assert.ok(distributionEmissionAccount.active, "Expected emission to be active")
    })

    it("Can stake an NFT", async () => {
      await stake(userProgram, staker.publicKey, nfts[3])
      await sleep(1000)
      const tokenEmissionAccount = await creatorProgram.account.emission.fetch(tokenEmission.publicKey)
      const distributionEmissionAccount = await creatorProgram.account.emission.fetch(distributionEmission.publicKey)

      assert.equal(tokenEmissionAccount.stakedItems.toNumber(), 1, "Expected 1 staked item")
      assert.equal(distributionEmissionAccount.stakedItems.toNumber(), 1, "Expected 1 staked item")
    })

    it("Can distribute", async () => {
      const stakeRecord = findStakeRecordPda(staker.publicKey, nfts[3].publicKey)
      await creatorProgram.methods
        .distribute(new BN(String(sol(10).basisPoints)))
        .accounts({
          staker: staker.publicKey,
          stakeRecord,
          collection,
          distributionEmission: distributionEmission.publicKey,
          nftMint: nfts[3].publicKey,
        })
        .rpc()
        .catch((err) => console.log(err))

      const stakeAccount = await creatorProgram.account.stakeRecord.fetch(stakeRecord)
      assert.equal(stakeAccount.solBalance.toString(), String(sol(10).basisPoints))
    })

    it("Can claim tokens", async () => {
      const balanceBefore = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)
      await claim(userProgram, staker.publicKey, nfts[3], tokenEmission.publicKey)
      const balanceAfter = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)

      assert.ok(balanceAfter > balanceBefore, "Expected balance to have increased")
    })

    it("Can claim sol", async () => {
      const balanceBefore = await umi.rpc.getBalance(user.publicKey)
      await claim(userProgram, staker.publicKey, nfts[3], distributionEmission.publicKey)
      const balanceAfter = await umi.rpc.getBalance(user.publicKey)

      assert.equal(
        balanceAfter.basisPoints - balanceBefore.basisPoints,
        BigInt(sol(10).basisPoints) - BASE_FEE - BigInt(programConfig.claimFee.toString()),
        "expected SOL to be withdrawn"
      )
    })

    it("Can distribute again", async () => {
      await sleep(1000)
      const stakeRecord = findStakeRecordPda(staker.publicKey, nfts[3].publicKey)
      await creatorProgram.methods
        .distribute(new BN(String(sol(10).basisPoints)))
        .accounts({
          staker: staker.publicKey,
          stakeRecord,
          collection,
          distributionEmission: distributionEmission.publicKey,
          nftMint: nfts[3].publicKey,
        })
        .rpc()
        .catch((err) => console.log(err))

      const stakeAccount = await creatorProgram.account.stakeRecord.fetch(stakeRecord)
      assert.equal(stakeAccount.solBalance.toString(), String(sol(10).basisPoints))
    })

    it("can unstake and claim", async () => {
      const balanceBefore = await umi.rpc.getBalance(user.publicKey)
      const acc = await umi.rpc.getAccount(findStakeRecordPda(staker.publicKey, nfts[3].publicKey))
      const tokenBefore = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)

      await unstake(userProgram, staker.publicKey, nfts[3])
      const tokenAfter = (await safeFetchToken(umi, getTokenAccount(token, user.publicKey)))?.amount || BigInt(0)

      const balanceAfter = await umi.rpc.getBalance(user.publicKey)

      const len = (acc.exists && acc.data.length) || 0
      const rentAmount = await umi.rpc.getRent(len)

      assert.ok(tokenAfter > tokenBefore, "expected tokens to be claimed")

      assert.equal(
        balanceAfter.basisPoints,
        balanceBefore.basisPoints +
          BigInt(sol(10).basisPoints) +
          rentAmount.basisPoints -
          BASE_FEE -
          BigInt(programConfig.unstakeFee.toString()),
        "expected SOL to be withdrawn"
      )
    })
  })
})
