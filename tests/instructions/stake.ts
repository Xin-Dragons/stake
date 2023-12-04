import { Keypair, PublicKey, sol, tokenAmount, unwrapOption } from "@metaplex-foundation/umi"
import { createCollection } from "../helpers/create-collection"
import { umi } from "../helpers/umi"
import { createNft } from "../helpers/create-nft"
import { toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters"
import {
  DigitalAsset,
  TokenState,
  fetchDigitalAsset,
  fetchDigitalAssetWithToken,
} from "@metaplex-foundation/mpl-token-metadata"
import {
  findNftAuthorityPda,
  findNftRecordPda,
  findStakeRecordPda,
  findStakooorCollectionId,
  getTokenAccount,
} from "../helpers/pdas"
import {
  AuthorityType,
  TokenState as LegacyState,
  createAssociatedToken,
  fetchToken,
  setAuthority,
} from "@metaplex-foundation/mpl-toolbox"
import { assert } from "chai"
import { BN } from "bn.js"
import { createToken } from "../helpers/create-token"
import { init, initCollection, stake } from "../helpers/instructions"
import { assertErrorCode, expectFail } from "../helpers/utils"
import { Program } from "@coral-xyz/anchor"
import { createNewUser, programPaidBy } from "../helper"
import { Stake } from "../../target/types/stake"

describe("Stake", () => {
  let creator: Keypair
  let creatorProgram: Program<Stake>
  let user: Keypair
  let userProgram: Program<Stake>
  let tokenMint: PublicKey
  let nftAuthority: PublicKey
  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  const slug = "stake_instruction"

  before(async () => {
    user = await createNewUser()
    userProgram = programPaidBy(user)
    creator = await createNewUser()
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

  describe("pNFT non-custodial", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)
      await initCollection(
        creatorProgram,
        stakerId,
        collection.publicKey,
        false,
        tokenMint,
        { transferToken: {} },
        1,
        0,
        undefined,
        3600
      )
    })

    it("Cannot stake someone elses NFT", async () => {
      const newUser = await createNewUser()
      const newUserProgram = programPaidBy(newUser)
      await createAssociatedToken(umi, {
        mint: nft.publicKey,
        owner: newUser.publicKey,
      }).sendAndConfirm(umi)

      await expectFail(
        () => stake(newUserProgram, stakerId, nft),
        (err) => assertErrorCode(err, "AccountNotInitialized")
      )
    })

    it("Can non-custodially stake a pNFT", async () => {
      await stake(userProgram, stakerId, nft)
      const stakooorCollection = await userProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(1)))

      const stakeRecord = findStakeRecordPda(stakerId, nft.publicKey)
      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)

      const record = await userProgram.account.stakeRecord.fetch(stakeRecord)
      assert.ok(record.owner.equals(userProgram.provider.publicKey))

      const stakerNftBalance = await fetchToken(umi, nftToken)
      assert.equal(stakerNftBalance.amount, BigInt(1), "Staker still holds the NFT")

      const nftAfter = await fetchDigitalAssetWithToken(umi, nft.publicKey, nftToken)
      assert.equal(nftAfter.tokenRecord.state, TokenState.Locked)
      assert.equal(unwrapOption(nftAfter.tokenRecord.delegate), nftAuthority)
    })
  })

  describe("pNFT custodial", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)
      await initCollection(creatorProgram, stakerId, collection.publicKey, true, tokenMint, { mintToken: {} })
    })

    it("Can custodially stake a pNFT", async () => {
      const stakeRecord = findStakeRecordPda(stakerId, nft.publicKey)
      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)
      const nftCustody = getTokenAccount(nft.publicKey, nftAuthority)

      await stake(userProgram, stakerId, nft)

      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)

      const stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(1)))

      const record = await userProgram.account.stakeRecord.fetch(stakeRecord)
      assert.ok(record.owner.equals(userProgram.provider.publicKey))

      const stakerNftBalance = await userProgram.provider.connection.getTokenAccountBalance(toWeb3JsPublicKey(nftToken))
      assert.equal(stakerNftBalance.value.uiAmount, 0, "Staker no longer holds the NFT")

      const custodyNftBalance = await userProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(nftCustody)
      )
      assert.equal(custodyNftBalance.value.uiAmount, 1, "Custody holds the NFT")
    })
  })

  describe("Legacy NFT non-custodial", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, false, collection.publicKey, user.publicKey)
      await initCollection(creatorProgram, stakerId, collection.publicKey, false, tokenMint, { mintToken: {} })
    })

    it("Can non-custodially stake a legacy NFT", async () => {
      const stakeRecord = findStakeRecordPda(stakerId, nft.publicKey)
      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)

      await stake(userProgram, stakerId, nft)

      const stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(1)))

      const record = await userProgram.account.stakeRecord.fetch(stakeRecord)
      assert.ok(record.owner.equals(userProgram.provider.publicKey))

      const stakerNftBalance = await userProgram.provider.connection.getTokenAccountBalance(toWeb3JsPublicKey(nftToken))
      assert.equal(stakerNftBalance.value.uiAmount, 1, "Staker still holds the NFT")

      const nftAfter = await fetchDigitalAssetWithToken(umi, nft.publicKey, nftToken)
      assert.equal(nftAfter.token.state, LegacyState.Frozen)
      assert.equal(unwrapOption(nftAfter.token.delegate), nftAuthority)
    })
  })

  describe("Legacy NFT custodial", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, false, collection.publicKey, user.publicKey)
      await initCollection(creatorProgram, stakerId, collection.publicKey, true, tokenMint, { mintToken: {} })
    })

    it("Can custodially stake a legacy NFT", async () => {
      const stakeRecord = findStakeRecordPda(stakerId, nft.publicKey)
      const nftToken = getTokenAccount(nft.publicKey, user.publicKey)
      const nftCustody = getTokenAccount(nft.publicKey, nftAuthority)

      await stake(userProgram, stakerId, nft)

      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)

      const stakooorCollection = await userProgram.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(1)))

      const record = await userProgram.account.stakeRecord.fetch(stakeRecord)
      assert.ok(record.owner.equals(userProgram.provider.publicKey))

      const stakerNftBalance = await userProgram.provider.connection.getTokenAccountBalance(toWeb3JsPublicKey(nftToken))
      assert.equal(stakerNftBalance.value.uiAmount, 0, "Staker no longer holds the NFT")

      const custodyNftBalance = await userProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(nftCustody)
      )
      assert.equal(custodyNftBalance.value.uiAmount, 1, "Custody holds the NFT")
    })
  })
})
