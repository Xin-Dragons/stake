import { Keypair, PublicKey, tokenAmount, unwrapOption } from "@metaplex-foundation/umi"
import {
  claim,
  closeCollection,
  init,
  initCollection,
  removeFunds,
  sleep,
  stake,
  unstake,
} from "../helpers/instructions"
import { createToken } from "../helpers/create-token"
import { umi } from "../helpers/umi"
import { assertErrorCode, expectFail, mintNfts } from "../helpers/utils"
import { findStakeRecordPda, findStakooorCollectionId, findTokenAuthorityPda, getTokenAccount } from "../helpers/pdas"
import { createCollection } from "../helpers/create-collection"
import {
  AuthorityType,
  createAssociatedToken,
  fetchMint,
  fetchToken,
  safeFetchToken,
  setAuthority,
} from "@metaplex-foundation/mpl-toolbox"
import { assert } from "chai"
import { DigitalAsset, updateV1 } from "@metaplex-foundation/mpl-token-metadata"
import { Program } from "@coral-xyz/anchor"
import { createNewUser, programPaidBy } from "../helper"
import { toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters"
import { Stake } from "../../target/types/stake"

describe("close collection", () => {
  const slug = "close_collection"
  let tokenMint: PublicKey

  let creator: Keypair
  let creatorProgram: Program<Stake>
  let tokenAuthority: PublicKey
  let user: Keypair
  let userProgram: Program<Stake>

  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  before(async () => {
    user = await createNewUser()
    userProgram = programPaidBy(user)
    creator = await createNewUser()
    creatorProgram = programPaidBy(creator)
    tokenAuthority = findTokenAuthorityPda(stakerId)
    tokenMint = await createToken(umi, tokenAmount(1_000_000, "token", 9).basisPoints, 9, undefined, creator.publicKey)
    await setAuthority(umi, {
      owned: tokenMint,
      owner: umi.identity.publicKey,
      authorityType: AuthorityType.MintTokens,
      newAuthority: creator.publicKey,
    }).sendAndConfirm(umi)
    await init(creatorProgram, keypair, slug)
  })

  describe("Token vault", () => {
    let collection: DigitalAsset
    let nfts: DigitalAsset[]
    let collectionId: PublicKey

    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 3, true, user.publicKey)
      collectionId = findStakooorCollectionId(stakerId, collection.publicKey)

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

    it("Cannot close the collection if not the creator", async () => {
      await createAssociatedToken(umi, {
        mint: tokenMint,
        owner: user.publicKey,
      }).sendAndConfirm(umi)

      await expectFail(
        () => closeCollection(userProgram, stakerId, collection.publicKey),
        (err) => assert.equal(err.error.errorCode.code, "Unauthorized")
      )
    })

    it("can close the collection", async () => {
      await stake(userProgram, stakerId, nfts[0])
      await stake(userProgram, stakerId, nfts[1])
      await sleep(1000)

      await closeCollection(creatorProgram, stakerId, collection.publicKey)
      const stakoorCollection = await creatorProgram.account.collection.fetch(collectionId)

      assert.equal(stakoorCollection.isActive, false, "expected stakooor collection to be closed")
    })

    it("cannot stake any more items", async () => {
      await expectFail(
        () => stake(userProgram, stakerId, nfts[2]),
        (err) => assert.equal(err.error.errorCode.code, "CollectionInactive")
      )
    })

    it("cannot claim", async () => {
      await expectFail(
        () => claim(userProgram, stakerId, nfts[1]),
        (err) => assert.equal(err.error.errorCode.code, "CollectionInactive")
      )
    })

    it("can unstake, claiming outstanding balance", async () => {
      const tokenAccount = getTokenAccount(tokenMint, user.publicKey)
      const balanceBefore = (await safeFetchToken(umi, tokenAccount))?.amount || BigInt(0)
      await unstake(userProgram, stakerId, nfts[0])
      await unstake(userProgram, stakerId, nfts[1])
      const balanceAfter = (await safeFetchToken(umi, tokenAccount))?.amount || BigInt(0)

      assert.ok(balanceAfter > balanceBefore, "Expected tokens to have been claimed")
    })

    it("can claim the outstanding balance", async () => {
      await removeFunds(creatorProgram, stakerId, collection.publicKey)
      const stakeVaultBalance = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      const coll = await creatorProgram.account.collection.fetch(collectionId)
      assert.equal(coll.currentBalance.toNumber(), 0, "Expected collection current balance to be 0")
      assert.equal(stakeVaultBalance.amount, BigInt(0), "Expected stake vault balance to be empty")
    })
  })

  describe("Mint tokens", () => {
    let collection: DigitalAsset
    let collection2: DigitalAsset
    let nfts: DigitalAsset[]
    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
      collection2 = await createCollection(umi)

      await initCollection(creatorProgram, stakerId, collection.publicKey, false, tokenMint, { mintToken: {} })
      await initCollection(creatorProgram, stakerId, collection2.publicKey, false, tokenMint, { mintToken: {} })
      await Promise.all(nfts.map((nft) => stake(userProgram, stakerId, nft)))
      // allow time for tokens to accrue
      await sleep(1000)
    })

    it("Cannot close a collection if there are more linked and they are not passed as additional accounts", async () => {
      await expectFail(
        () => closeCollection(creatorProgram, stakerId, collection.publicKey),
        (err) => assertErrorCode(err, "CollectionsMissing")
      )
    })

    it("Can close a collection with mint auth if all collections are passed as additional accounts", async () => {
      const stakooor = await creatorProgram.account.staker.fetch(stakerId)
      const balanceBefore = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      const sig = await closeCollection(creatorProgram, stakerId, collection.publicKey, stakooor.collections)
      const stakooorCollection = await creatorProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )

      const balanceAfter = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))

      assert.ok(!stakooorCollection.isActive, "Expected collection to have been deactivated")
      assert.ok(stakooorCollection.rewardType.transferToken, "Expected reward type to be updated to token vault")
      const mint = await fetchMint(umi, tokenMint)
      assert.equal(
        unwrapOption(mint.mintAuthority),
        tokenAuthority,
        "Expected mintAuthority to remain with the program"
      )

      const stakeRecord = await creatorProgram.account.stakeRecord.fetch(
        findStakeRecordPda(stakerId, nfts[0].publicKey)
      )

      const stakedAt = BigInt(stakeRecord.stakedAt.toNumber())
      const unstakedAt = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: sig }))

      const timeStaked = unstakedAt - stakedAt

      assert.equal(balanceBefore.amount, BigInt(0), "Expected no token balance before")
      assert.ok(
        balanceAfter.amount >= BigInt(nfts.length) * timeStaked,
        "Expected tokens to be minted to the token vault for accrued rewards"
      )
      assert.equal(
        BigInt(stakooorCollection.currentBalance.toString()),
        balanceAfter.amount,
        "Expected collection current balance to reflect actual balance"
      )
    })

    it("no balance can be withdrawn if there are still all items staked", async () => {
      await expectFail(
        () => removeFunds(creatorProgram, stakerId, collection.publicKey),
        (err) => assertErrorCode(err, "CollectionHasStakers")
      )
    })

    it("remaining balance can be withdrawn if all items are unstaked", async () => {
      await Promise.all(nfts.map((nft) => unstake(userProgram, stakerId, nft)))

      const balanceBefore = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      await removeFunds(creatorProgram, stakerId, collection.publicKey)
      const balanceAfter = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      assert.equal(balanceAfter.amount, BigInt(0), "Expected collection balance to be emptied")

      const stakooorCollection = await creatorProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      assert.equal(stakooorCollection.currentBalance.toNumber(), 0, "Expected balance to be zero")
    })

    it("can close the other collection, revoking mint auth", async () => {
      await closeCollection(creatorProgram, stakerId, collection2.publicKey)
      const mint = await fetchMint(umi, tokenMint)
      assert.equal(
        unwrapOption(mint.mintAuthority),
        creator.publicKey,
        "Expected mint auth to be transferred back to the creator"
      )
    })
  })

  describe("Complex multi-collection setup", () => {
    let collection1: DigitalAsset
    let collection2: DigitalAsset
    let collection3: DigitalAsset
    let nfts1: DigitalAsset[]
    let nfts2: DigitalAsset[]
    let nfts3: DigitalAsset[]
    before(async () => {
      collection1 = await createCollection(umi)
      collection2 = await createCollection(umi)
      collection3 = await createCollection(umi)
      nfts1 = await mintNfts(collection1.publicKey, 10, true, user.publicKey)
      nfts2 = await mintNfts(collection2.publicKey, 10, false, user.publicKey)
      nfts3 = await mintNfts(collection3.publicKey, 10, true, user.publicKey)

      await updateV1(umi, {
        mint: collection1.publicKey,
        newUpdateAuthority: creator.publicKey,
      }).sendAndConfirm(umi)

      await initCollection(
        creatorProgram,
        stakerId,
        collection1.publicKey,
        false,
        tokenMint,
        { transferToken: {} },
        10,
        3600,
        undefined,
        3600,
        true
      )

      await initCollection(creatorProgram, stakerId, collection2.publicKey, false, tokenMint, { mintToken: {} }, 3)
      await initCollection(creatorProgram, stakerId, collection3.publicKey, false, tokenMint, { mintToken: {} }, 17)

      await Promise.all([...nfts1, ...nfts2, ...nfts3].map((nft) => stake(userProgram, stakerId, nft)))
    })

    it("cannot unstake an item staked with a min term", async () => {
      await expectFail(
        () => unstake(userProgram, stakerId, nfts1[0]),
        (err) => assertErrorCode(err, "MinimumPeriodNotReached")
      )
    })

    it("can unstake from collection 2", async () => {
      // allow tokens to accrue
      await sleep(1000)
      const stakeRecord = await userProgram.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nfts2[0].publicKey))

      const balanceBefore = (await safeFetchToken(umi, getTokenAccount(tokenMint, user.publicKey)))?.amount || BigInt(0)
      const sig = await unstake(userProgram, stakerId, nfts2[0])
      const balanceAfter = await fetchToken(umi, getTokenAccount(tokenMint, user.publicKey))
      const blockTime = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: sig }))
      const timeStaked = blockTime - BigInt(stakeRecord.stakedAt.toString())

      const expectedTokens = timeStaked * BigInt(3)
      assert.equal(
        balanceAfter.amount - balanceBefore,
        expectedTokens,
        "Expected to receive the correct amount of tokens"
      )
    })

    it("cannot close without passing all collections in remaining accounts", async () => {
      await expectFail(
        () => closeCollection(creatorProgram, stakerId, collection2.publicKey),
        (err) => assertErrorCode(err, "CollectionsMissing")
      )

      await expectFail(
        () =>
          closeCollection(creatorProgram, stakerId, collection2.publicKey, [
            toWeb3JsPublicKey(findStakooorCollectionId(stakerId, collection1.publicKey)),
          ]),
        (err) => assertErrorCode(err, "CollectionsMissing")
      )

      await expectFail(
        () =>
          closeCollection(creatorProgram, stakerId, collection2.publicKey, [
            toWeb3JsPublicKey(findStakooorCollectionId(stakerId, collection3.publicKey)),
          ]),
        (err) => assertErrorCode(err, "CollectionsMissing")
      )
    })

    it("cannot withdraw the remaining balance if collection still live", async () => {
      await expectFail(
        () => removeFunds(creatorProgram, stakerId, collection1.publicKey),
        (err) => assertErrorCode(err, "CollectionActive")
      )
    })

    it("can close staking for collection 2", async () => {
      const stakeRecord = await userProgram.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nfts2[1].publicKey))
      const stakooor = await userProgram.account.staker.fetch(stakerId)
      const sig = await closeCollection(creatorProgram, stakerId, collection2.publicKey, stakooor.collections)
      const blockTime = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: sig }))
      const balAfter = await safeFetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))

      const timeStaked = blockTime - BigInt(stakeRecord.stakedAt.toString())
      const expectedOwingEmission = timeStaked * BigInt(3) * BigInt(9)

      assert.ok(
        balAfter.amount >= expectedOwingEmission,
        "Expected to have minted enough tokens for the remainder to claim"
      )
    })

    it("cannot withdraw the remaining balance if items still staked", async () => {
      await expectFail(
        () => removeFunds(creatorProgram, stakerId, collection2.publicKey),
        (err) => assertErrorCode(err, "CollectionHasStakers")
      )
    })

    it("can withdraw the remaining balance once all users have unstaked", async () => {
      const balBeginning = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))

      const startDates = await Promise.all(
        nfts2.slice(1).map(async (nft) => {
          const stakeRecordPK = findStakeRecordPda(stakerId, nft.publicKey)
          const stakeRecord = await userProgram.account.stakeRecord.fetch(stakeRecordPK)
          return BigInt(stakeRecord.stakedAt.toNumber())
        })
      )

      await Promise.all(nfts2.slice(1).map((nft) => unstake(userProgram, stakerId, nft)))

      const balanceBefore = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      const collectionBefore = await userProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection2.publicKey)
      )
      await removeFunds(creatorProgram, stakerId, collection2.publicKey)
      const balanceAfter = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      const collectionAfter = await userProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection2.publicKey)
      )

      assert.ok(balanceBefore.amount > balanceAfter.amount, "Expected amount to have been claimed")

      assert.equal(
        balanceBefore.amount - balanceAfter.amount,
        BigInt(collectionBefore.currentBalance.toString()),
        "Expected full balance to be claimed"
      )

      assert.equal(collectionAfter.currentBalance.toNumber(), 0, "Expected collection balance to be marked as 0")

      assert.equal(
        balanceAfter.amount,
        BigInt(3600) * BigInt(10) * BigInt(10),
        "Expected tokens for collection1 emissions to remain in vault"
      )
    })

    it("can close collection 1, claiming back the surplus tokens", async () => {
      const stakeRecord = await userProgram.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nfts1[0].publicKey))
      const balanceBefore = await fetchToken(umi, getTokenAccount(tokenMint, creator.publicKey))
      const stakooor = await userProgram.account.staker.fetch(stakerId)
      const sig = await closeCollection(creatorProgram, stakerId, collection1.publicKey, stakooor.collections)
      const balanceAfter = await fetchToken(umi, getTokenAccount(tokenMint, creator.publicKey))
      const blockTime = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: sig }))
      const timeStaked = blockTime - BigInt(stakeRecord.stakedAt.toString())
      const vaultBalance = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))

      assert.ok(balanceAfter.amount > balanceBefore.amount, "Expected to have withdrawn some tokens")

      const expectedEmission = timeStaked * BigInt(10) * BigInt(10)
      assert.ok(vaultBalance.amount >= expectedEmission, "Expected to have left enough to claim, with some leeway")
    })

    it("can unstake all remaining items and claim back all tokens, skipping min period", async () => {
      await Promise.all(nfts1.map((nft) => unstake(userProgram, stakerId, nft)))
      await removeFunds(creatorProgram, stakerId, collection1.publicKey)

      const balance = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      assert.equal(balance.amount, BigInt(0), "Expected all tokens to have been withdrawn")
    })

    it("can still claim from collection3, minting new tokens", async () => {
      const mintBefore = await fetchMint(umi, tokenMint)
      const stakeRecord = await userProgram.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nfts3[0].publicKey))
      const sig = await claim(userProgram, stakerId, nfts3[0])
      const mintAfter = await fetchMint(umi, tokenMint)
      const blockTime = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: sig }))
      const timeStaked = blockTime - BigInt(stakeRecord.stakedAt.toString())

      const expectedTokens = timeStaked * BigInt(17)
      assert.equal(
        mintAfter.supply - mintBefore.supply,
        expectedTokens,
        "Expected to have minted the correct amount of tokens"
      )
    })

    it("can close last collection and get back mint auth", async () => {
      await Promise.all(nfts3.map((nft) => unstake(userProgram, stakerId, nft)))
      await closeCollection(creatorProgram, stakerId, collection3.publicKey)

      const mint = await fetchMint(umi, tokenMint)
      const balance = await fetchToken(umi, getTokenAccount(tokenMint, tokenAuthority))
      assert.equal(balance.amount, BigInt(0), "Expected all tokens to have been withdrawn")
      assert.equal(
        unwrapOption(mint.mintAuthority),
        creator.publicKey,
        "Expected mint auth to have been returned to creator"
      )
    })
  })
})
