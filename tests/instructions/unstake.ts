import { createCollection } from "../helpers/create-collection"
import { umi } from "../helpers/umi"
import { createNft } from "../helpers/create-nft"
import { createToken } from "../helpers/create-token"
import { init, initCollection, sleep, stake, unstake } from "../helpers/instructions"
import {
  AuthorityType,
  TokenState as LegacyState,
  fetchToken,
  safeFetchToken,
  setAuthority,
} from "@metaplex-foundation/mpl-toolbox"
import { findNftAuthorityPda, getTokenAccount, findStakooorCollectionId } from "../helpers/pdas"
import {
  DigitalAsset,
  TokenDelegateRole,
  TokenState,
  fetchDigitalAssetWithAssociatedToken,
} from "@metaplex-foundation/mpl-token-metadata"
import { fromWeb3JsPublicKey, toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters"
import { assert } from "chai"
import { Keypair, PublicKey, isNone, tokenAmount, unwrapOption } from "@metaplex-foundation/umi"
import { Program } from "@coral-xyz/anchor"
import { createNewUser, programPaidBy } from "../helper"
import { assertErrorCode, expectFail } from "../helpers/utils"
import { Stake } from "../../target/types/stake"

describe("Unstake", () => {
  const slug = "unstake_instruction"
  let user: Keypair
  let creator: Keypair
  let userProgram: Program<Stake>
  let creatorProgram: Program<Stake>
  let tokenMint: PublicKey
  let nftAuthority: PublicKey
  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  before(async () => {
    user = await createNewUser()
    creator = await createNewUser()
    userProgram = programPaidBy(user)
    creatorProgram = programPaidBy(creator)
    tokenMint = await createToken(umi, tokenAmount(10_000, "token", 9).basisPoints, 9, undefined, creator.publicKey)

    await setAuthority(umi, {
      owned: tokenMint,
      owner: umi.identity.publicKey,
      authorityType: AuthorityType.MintTokens,
      newAuthority: creator.publicKey,
    }).sendAndConfirm(umi)

    nftAuthority = findNftAuthorityPda(stakerId)
    await init(creatorProgram, keypair, slug)
  })

  describe("Non custodial pNFT", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)

      await initCollection(creatorProgram, stakerId, collection.publicKey, false, tokenMint, { mintToken: {} })
      await stake(userProgram, stakerId, nft)
    })

    it("cannot unstake somone elses nft", async () => {
      const newUser = await createNewUser()
      const newUserProgram = programPaidBy(newUser)
      await expectFail(
        () => unstake(newUserProgram, stakerId, nft),
        (err) => assertErrorCode(err, "Unauthorized")
      )
    })

    it("can unstake a locked pNFT", async () => {
      // wait for 1 second for tokens to accrue
      await sleep(1000)
      const rewardReceiveAccount = getTokenAccount(tokenMint, user.publicKey)
      let nftWithToken = await fetchDigitalAssetWithAssociatedToken(umi, nft.publicKey, user.publicKey)
      assert.equal(
        unwrapOption(nftWithToken.tokenRecord.delegate),
        nftAuthority,
        "Expected NFT authority to be delegated"
      )

      assert.equal(nftWithToken.tokenRecord.state, TokenState.Locked, "Expected NFT to be locked")
      assert.equal(
        unwrapOption(nftWithToken.tokenRecord.delegateRole),
        TokenDelegateRole.Utility,
        "Expected Utility role to be assigned"
      )

      const balanceBefore = (await safeFetchToken(umi, rewardReceiveAccount))?.amount || BigInt(0)

      await unstake(userProgram, stakerId, nft)

      const balanceAfter = (await safeFetchToken(umi, rewardReceiveAccount))?.amount || BigInt(0)
      assert.ok(balanceAfter > balanceBefore)

      nftWithToken = await fetchDigitalAssetWithAssociatedToken(
        umi,
        nft.publicKey,
        fromWeb3JsPublicKey(userProgram.provider.publicKey)
      )

      assert.equal(nftWithToken.tokenRecord.state, TokenState.Unlocked, "Expected NFT to be unlocked")
      assert.ok(isNone(nftWithToken.tokenRecord.delegate), "Expected NFT delegate to be cleared")
    })
  })

  describe("Non custodial legacy NFT", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, false, collection.publicKey, user.publicKey)

      await initCollection(creatorProgram, stakerId, collection.publicKey, false, tokenMint, { mintToken: {} })
      await stake(userProgram, stakerId, nft)
    })

    it("can unstake a locked legacy NFT", async () => {
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
      const rewardReceiveAccount = getTokenAccount(tokenMint, user.publicKey)

      let stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)

      // wait 1 second for tokens to accrue
      await sleep(1000)

      let nftWithToken = await fetchDigitalAssetWithAssociatedToken(
        umi,
        nft.publicKey,
        fromWeb3JsPublicKey(userProgram.provider.publicKey)
      )

      assert.equal(nftWithToken.token.state, LegacyState.Frozen, "Expected NFT to be frozen (legacy)")
      assert.equal(
        unwrapOption(nftWithToken.token.delegate),
        nftAuthority,
        "Expected delegate to be set as NFT Authority"
      )

      stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)

      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 1, "Expected stakooor to have 1 staker")

      const balanceBefore = (await safeFetchToken(umi, rewardReceiveAccount))?.amount || BigInt(0)

      await unstake(userProgram, stakerId, nft)

      const balanceAfter = (await safeFetchToken(umi, rewardReceiveAccount))?.amount || BigInt(0)
      stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)

      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 0, "Expected stakooor to have 0 stakers")

      assert.ok(balanceAfter > balanceBefore, "Expected balance after unstake to be higher than balance before unstake")

      nftWithToken = await fetchDigitalAssetWithAssociatedToken(
        umi,
        nft.publicKey,
        fromWeb3JsPublicKey(userProgram.provider.publicKey)
      )

      assert.equal(nftWithToken.token.state, LegacyState.Initialized, "Expected NFT to be unlocked")
      assert.ok(isNone(nftWithToken.token.delegate), "Expected NFT delegate to be cleared")
      assert.equal(nftWithToken.token.delegatedAmount, BigInt(0))
    })
  })

  describe("Custodial pNFT", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)

      await initCollection(creatorProgram, stakerId, collection.publicKey, true, tokenMint, { mintToken: {} })
      await stake(userProgram, stakerId, nft)
    })

    it("can unstake a locked pNFT", async () => {
      const rewardReceiveAccount = getTokenAccount(tokenMint, user.publicKey)
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
      // wait for 1 second for tokens to accrue
      await sleep(1000)

      let stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)

      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 1, "Expected 1 staker")

      const nftCustody = getTokenAccount(nft.publicKey, nftAuthority)
      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)

      let [sourceBalance, destinationBalance] = await Promise.all([
        fetchToken(umi, nftToken),
        fetchToken(umi, nftCustody),
      ])

      assert.equal(sourceBalance.amount, BigInt(0))
      assert.equal(destinationBalance.amount, BigInt(1))

      const balanceBefore = await userProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(rewardReceiveAccount)
      )

      await unstake(userProgram, stakerId, nft)

      stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      const balance = await userProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(rewardReceiveAccount)
      )

      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 0, "Expected stakooor to have 0 stakers")

      assert.ok(
        balance.value.uiAmount > balanceBefore.value.uiAmount,
        "Expected balance after unstake to be higher than balance before unstake"
      )

      assert.ok(balance.value.uiAmount > balanceBefore.value.uiAmount)
      sourceBalance = await fetchToken(umi, nftToken)

      assert.equal(sourceBalance.amount, BigInt(1), "Expected NFT to be returned to holder")

      const destinationAcc = await umi.rpc.getAccount(nftCustody)

      assert.ok(!destinationAcc.exists, "Expected custody account to be closed")
    })
  })

  describe("Custodial legacy NFT", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, false, collection.publicKey, user.publicKey)

      await initCollection(creatorProgram, stakerId, collection.publicKey, true, tokenMint, { mintToken: {} })
      await stake(userProgram, stakerId, nft)
    })

    it("can unstake a custodial legacy NFT", async () => {
      const rewardReceiveAccount = getTokenAccount(tokenMint, user.publicKey)
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
      // wait for 1 second for tokens to accrue
      await sleep(1000)

      let stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 1, "Expected stakooor to have 1 staker")

      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)
      const nftCustody = getTokenAccount(nft.publicKey, nftAuthority)

      let [sourceBalance, destinationBalance] = await Promise.all([
        fetchToken(umi, nftToken),
        fetchToken(umi, nftCustody),
      ])

      assert.equal(sourceBalance.amount, BigInt(0))
      assert.equal(destinationBalance.amount, BigInt(1))

      const balanceBefore = await fetchToken(umi, rewardReceiveAccount)

      await unstake(userProgram, stakerId, nft)

      stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      const balanceAfter = await fetchToken(umi, rewardReceiveAccount)

      assert.equal(stakooorCollection.currentStakersCount.toNumber(), 0, "Expected stakooor to have 0 stakers")

      assert.ok(
        balanceAfter.amount > balanceBefore.amount,
        "Expected balance after unstake to be higher than balance before unstake"
      )

      sourceBalance = await fetchToken(umi, nftToken)

      assert.equal(sourceBalance.amount, BigInt(1), "Expected NFT to be returned to holder")

      const destinationAcc = await umi.rpc.getAccount(nftCustody)

      assert.ok(!destinationAcc.exists, "Expected custody account to be closed")
    })
  })
})
