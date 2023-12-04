import * as anchor from "@coral-xyz/anchor"
import { DigitalAsset, TokenStandard, transferV1, updateV1 } from "@metaplex-foundation/mpl-token-metadata"
import { createCollection } from "../helpers/create-collection"
import { FEES_WALLET, USDC, assertErrorCode, expectFail, getStakedItemsForUser, mintNfts } from "../helpers/utils"
import {
  addFunds,
  changeReward,
  claim,
  closeCollection,
  init,
  initCollection,
  sleep,
  stake,
  unstake,
} from "../helpers/instructions"
import { createToken } from "../helpers/create-token"
import { assert } from "chai"
import { BN } from "bn.js"
import {
  Keypair,
  PublicKey,
  createSignerFromKeypair,
  generateSigner,
  tokenAmount,
  unwrapOption,
} from "@metaplex-foundation/umi"
import {
  AuthorityType,
  TokenState as LegacyState,
  fetchMint,
  fetchToken,
  mintTokensTo,
  safeFetchToken,
  setAuthority,
} from "@metaplex-foundation/mpl-toolbox"
import {
  findNftAuthorityPda,
  findNftRecordPda,
  findProgramConfigPda,
  findStakeRecordPda,
  findStakooorCollectionId,
  findTokenAuthorityPda,
  getTokenAccount,
} from "../helpers/pdas"
import { toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters"
import { createNft } from "../helpers/create-nft"
import { createNewUser, programPaidBy } from "../helper"
import { umi } from "../helpers/umi"
import { Stake } from "../../target/types/stake"

describe("E2E tests", () => {
  const slug = "e2e_staker"
  let tokenMint: PublicKey
  let stakeVaultAccount: PublicKey
  let owner: Keypair
  let user: Keypair
  let user2: Keypair
  let ownerProgram: anchor.Program<Stake>

  let user1Program: anchor.Program<Stake>
  let user2Program: anchor.Program<Stake>

  let tokenAuth: PublicKey
  let nftAuthority: PublicKey

  const keypair = umi.eddsa.generateKeypair()
  const stakerId = keypair.publicKey

  before(async () => {
    owner = await createNewUser()
    user = await createNewUser()
    user2 = await createNewUser()
    ownerProgram = programPaidBy(owner)
    user1Program = programPaidBy(user)
    user2Program = programPaidBy(user2)
    tokenAuth = findTokenAuthorityPda(stakerId)
    nftAuthority = findNftAuthorityPda(stakerId)
    tokenMint = await createToken(umi, BigInt(10_000), 9, generateSigner(umi), owner.publicKey)

    await setAuthority(umi, {
      authorityType: AuthorityType.MintTokens,
      owned: tokenMint,
      owner: umi.identity.publicKey,
      newAuthority: owner.publicKey,
    }).sendAndConfirm(umi)

    await init(ownerProgram, keypair, slug)
    stakeVaultAccount = getTokenAccount(tokenMint, tokenAuth)
  })

  it("Is initialized without a collection", async () => {
    const stakooor = await ownerProgram.account.staker.fetch(stakerId)
    assert.equal(stakooor.collections.length, 0, "expected collections to be empty")
  })

  describe("Future start date", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)
    })

    it("can add a collection which doesn't start right away", async () => {
      const startTime = new BN(Math.floor(Date.now() / 1000) + 1)
      await initCollection(
        ownerProgram,
        stakerId,
        collection.publicKey,
        true,
        tokenMint,
        {
          transferToken: {},
        },
        1,
        0,
        startTime,
        2
      )

      const collectionAccount = await ownerProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      assert.ok(collectionAccount.stakingStartsAt.eq(startTime), "Expected start time to be set")
      assert.ok(collectionAccount.stakingEndsAt.eq(startTime.add(new BN(2))), "Expected end time to be set")
    })

    it("Cannot stake an NFT if staking hasn't started yet", async () => {
      await expectFail(
        () => stake(user1Program, stakerId, nft),
        (err) => {
          assertErrorCode(err, "StakeNotLive")
        }
      )
    })

    it("Can stake once the staking is live", async () => {
      await sleep(2000)
      await stake(user1Program, stakerId, nft)

      const stakeRecord = await ownerProgram.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nft.publicKey))
      assert.ok(stakeRecord, "Expected stake record to exist")
    })

    it("Cannot stake after staking is over", async () => {
      await unstake(user1Program, stakerId, nft)
      await sleep(2000)
      await expectFail(
        () => stake(user1Program, stakerId, nft),
        (err) => assertErrorCode(err, "StakeOver")
      )
    })
  })

  describe("Custodial pNFT token mint collection", () => {
    let collection: DigitalAsset
    let nfts: DigitalAsset[]
    let stakooorCollectionId: PublicKey

    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
      stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
    })

    it("cannot stake an NFT with no collection", async () => {
      await expectFail(
        () => stake(user1Program, stakerId, nfts[0]),
        (err) => {
          assert.equal(err.error.errorCode.code, "AccountNotInitialized")
        }
      )
    })

    it("Can add a collection to the stakooor", async () => {
      await initCollection(ownerProgram, stakerId, collection.publicKey, true, tokenMint, {
        mintToken: {},
      })
      const stakooor = await ownerProgram.account.staker.fetch(stakerId)

      assert.ok(
        stakooor.collections.find(
          (coll) => coll.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey)
        ),
        "expected collection to be added"
      )
      assert.equal(
        stakooor.collections[stakooor.collections.length - 1].toBase58(),
        findStakooorCollectionId(stakerId, collection.publicKey),
        "expected correct collection to be referenced"
      )
    })

    it("transfers mint auth to the tokenAuthority", async () => {
      const mint = await fetchMint(umi, tokenMint)
      assert.equal(unwrapOption(mint.mintAuthority), tokenAuth, "Expected mint auth to be delegated")
    })

    it("can stake an NFT", async () => {
      await stake(user1Program, stakerId, nfts[0])
      const stakooorCollection = await user1Program.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(1)), "Expected 1 staker")
    })

    it("sends the token to the nftCustody escrow", async () => {
      const nftMint = nfts[0].publicKey
      const nftToken = getTokenAccount(nftMint, user.publicKey)
      const nftCustody = getTokenAccount(nftMint, nftAuthority)

      const holderAcc = await fetchToken(umi, nftToken)
      assert.equal(holderAcc.amount, BigInt(0), "Expected balance of holder NFT account to be zero")

      const custodyAccount = await fetchToken(umi, nftCustody)
      assert.equal(custodyAccount.amount, BigInt(1), "Expected balance of holder NFT account to be 1")
    })

    it("cannot stake the same token again", async () => {
      await expectFail(
        () => stake(user1Program, stakerId, nfts[0]),
        (err) => {
          assert.equal(err.error.errorCode.code, "AccountNotInitialized")
        }
      )
    })

    it("can stake another NFT", async () => {
      await stake(user1Program, stakerId, nfts[1])
      const stakooorCollection = await user1Program.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(2)), "Expected 2 stakers")
      const stakedItems = await getStakedItemsForUser(user1Program, user.publicKey)
      assert.equal(stakedItems.length, 2, "Expected 2 staked items for user")
    })

    it("can stake NFT for a new user", async () => {
      const nft = nfts[2]

      await expectFail(
        () => stake(user2Program, stakerId, nft),
        (err) => {
          assert.equal(err.error.errorCode.code, "AccountNotInitialized")
        }
      )

      await transferV1(umi, {
        mint: nft.publicKey,
        token: getTokenAccount(nft.publicKey, user.publicKey),
        tokenOwner: user.publicKey,
        tokenStandard: TokenStandard.ProgrammableNonFungible,
        destinationToken: getTokenAccount(nft.publicKey, user2.publicKey),
        destinationOwner: user2.publicKey,
        authority: createSignerFromKeypair(umi, user),
      }).sendAndConfirm(umi)

      await stake(user2Program, stakerId, nft)

      const stakooorCollection = await user2Program.account.collection.fetch(stakooorCollectionId)
      assert.ok(stakooorCollection.currentStakersCount.eq(new BN(3)), "Expected 3 items staked")

      const user1Staked = await getStakedItemsForUser(ownerProgram, user.publicKey)
      assert.equal(user1Staked.length, 2, "Expected 2 staked items for user1")

      const user2Staked = await getStakedItemsForUser(ownerProgram, user2.publicKey)
      assert.equal(user2Staked.length, 1, "Expected 1 staked items for user2")
    })

    it("can claim owing tokens", async () => {
      const stakeRecordId = findStakeRecordPda(stakerId, nfts[0].publicKey)
      let stakeRecordAccount = await user1Program.account.stakeRecord.fetch(stakeRecordId)

      const rewardReceiveAccount = getTokenAccount(tokenMint, user.publicKey)

      const stakedAt = stakeRecordAccount.stakedAt

      const fees = await user1Program.account.programConfig.fetch(findProgramConfigPda())
      const feeBalanceBefore = await user1Program.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))

      const balanceBefore = await fetchToken(umi, rewardReceiveAccount)
      await claim(user1Program, stakerId, nfts[0])
      const feeBalanceAfter = await user1Program.provider.connection.getBalance(toWeb3JsPublicKey(FEES_WALLET))
      assert.equal(
        feeBalanceAfter - feeBalanceBefore,
        fees.claimFee.toNumber(),
        "Expected to have paid the correct fee"
      )

      stakeRecordAccount = await user1Program.account.stakeRecord.fetch(stakeRecordId)
      const stakooorCollection = await user1Program.account.collection.fetch(stakooorCollectionId)

      const claimedAt = stakeRecordAccount.stakedAt
      const duration = claimedAt.sub(stakedAt)

      const balanceAfter = await fetchToken(umi, rewardReceiveAccount)

      const tokensClaimed = balanceAfter.amount - balanceBefore.amount
      assert.equal(
        Number(tokensClaimed),

        duration.mul(stakooorCollection.reward.pop()).toNumber(),
        "expected correct emission"
      )
      assert.ok(claimedAt > stakedAt, "Staked at should be updated on claim")
    })

    it("cannot claim someone elses tokens", async () => {
      await expectFail(
        () => claim(user2Program, stakerId, nfts[0]),
        (err) => {
          assert.equal(err.error.errorCode.code, "Unauthorized")
        }
      )
    })

    it("cannot unstake someone elses NFT", async () => {
      const program = programPaidBy(user2)

      await expectFail(
        () => unstake(program, stakerId, nfts[0]),
        (err) => {
          assert.equal(err.error.errorCode.code, "Unauthorized")
        }
      )
    })

    it("can stake the remaining nfts", async () => {
      const toStake = nfts.slice(3)
      await Promise.all(toStake.map((nft) => stake(user1Program, stakerId, nft)))
      const staked = await getStakedItemsForUser(user1Program, user.publicKey)
      assert.equal(staked.length, 9, "expected 9 items to be staked for user")
    })

    it("cannot stake a newly minted NFT", async () => {
      const nft = await createNft(umi, true, collection.publicKey, user.publicKey)
      await expectFail(
        () => stake(user1Program, stakerId, nft),
        (err) => {
          assert.equal(err.error.errorCode.code, "MaxStakersReached")
        }
      )
    })
  })

  describe("Custodial NFT collection - same token", () => {
    let nfts: DigitalAsset[]
    let collection: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, false, user.publicKey)
    })

    it("Can add a collection to the stakooor", async () => {
      await initCollection(ownerProgram, stakerId, collection.publicKey, true, tokenMint, {
        mintToken: {},
      })
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
      const stakooor = await ownerProgram.account.staker.fetch(stakerId)

      assert.ok(
        stakooor.collections.find(
          (coll) => coll.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey)
        ),
        "expected collection to be added"
      )
      assert.ok(
        stakooor.collections.find((coll) => coll.toBase58() === stakooorCollectionId),
        "expected collection to be added to stakooor"
      )
    })

    it("can stake a legacy NFT", async () => {
      const nft = nfts[0]
      await stake(user1Program, stakerId, nft)
      const stakeRecord = await user1Program.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nft.publicKey))
      assert.equal(stakeRecord.nftMint.toBase58(), nft.publicKey)
      assert.equal(stakeRecord.owner.toBase58(), user.publicKey)
      assert.ok(stakeRecord.stakedAt.toNumber() < Date.now() / 1000)

      const ownerToken = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      const custodyToken = await fetchToken(umi, getTokenAccount(nft.publicKey, nftAuthority))

      assert.equal(ownerToken.amount, BigInt(0), "expected holder to no longer hold NFT")
      assert.equal(custodyToken.amount, BigInt(1), "expected custody to hold NFT")
    })

    it("can unstake a legacy NFT", async () => {
      const nft = nfts[0]
      const tokenAccount = getTokenAccount(tokenMint, user.publicKey)
      const balanceBefore = (await safeFetchToken(umi, tokenAccount)).amount || BigInt(0)
      await sleep(1_000)
      await unstake(user1Program, stakerId, nft)
      const balanceAfter = (await fetchToken(umi, tokenAccount)).amount
      assert.ok(balanceBefore < balanceAfter, "expected tokens to be claimed")

      const tokenAcc = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      assert.equal(tokenAcc.amount, BigInt(1), "Expected to receive NFT back")
    })
  })

  describe("Collection with min-period (non-enforced)", () => {
    let nfts: DigitalAsset[]
    let collection: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
    })

    it("can add a new NFT collection, with a min period of an hour", async () => {
      const reward = 10
      const minimumPeriod = 1 * 60 * 60
      const duration = 1 * 60 * 60

      const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())
      const usdcSubAccount = getTokenAccount(USDC.publicKey, FEES_WALLET)

      const usdcBalanceBefore = (await safeFetchToken(umi, usdcSubAccount))?.amount ?? BigInt(0)

      const balanceBefore = (await safeFetchToken(umi, stakeVaultAccount))?.amount ?? BigInt(0)
      await initCollection(
        ownerProgram,
        stakerId,
        collection.publicKey,
        true,
        tokenMint,
        { mintToken: {} },
        reward,
        minimumPeriod,
        null,
        duration
      )

      const balanceAfter = (await fetchToken(umi, stakeVaultAccount)).amount

      const usdcBalanceAfter = (await fetchToken(umi, usdcSubAccount)).amount

      assert.equal(balanceBefore, balanceAfter, "expected no tokens to be transferred")

      assert.equal(
        usdcBalanceBefore + BigInt(programConfig.extraCollectionFee.toNumber()),
        usdcBalanceAfter,
        "expected to pay for additional collection"
      )

      const stakooor = await ownerProgram.account.staker.fetch(stakerId)
      assert.ok(
        stakooor.collections.find(
          (coll) => coll.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey)
        ),
        "expected collection to be added"
      )
      assert.ok(
        stakooor.collections.find((c) => c.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey))
      )
    })

    it("can stake an NFT", async () => {
      const nft = nfts[0]
      await stake(user1Program, stakerId, nft)
      const stakeRecord = await user1Program.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nft.publicKey))
      assert.equal(stakeRecord.nftMint.toBase58(), nft.publicKey)
      assert.equal(stakeRecord.owner.toBase58(), user.publicKey)
      assert.ok(stakeRecord.stakedAt.toNumber() < Date.now() / 1000)

      const ownerToken = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      const custodyToken = await fetchToken(umi, getTokenAccount(nft.publicKey, nftAuthority))

      assert.equal(ownerToken.amount, BigInt(0), "expected holder to no longer hold NFT")
      assert.equal(custodyToken.amount, BigInt(1), "expected custody to hold NFT")
    })

    it("cannot claim if min period not exceeded", async () => {
      const nft = nfts[0]

      await expectFail(
        () => claim(user1Program, stakerId, nft),
        (err) => assert.equal(err.error.errorCode.code, "MinimumPeriodNotReached")
      )
    })

    it("can unstake if min period not exceeded, no tokens received", async () => {
      const nft = nfts[0]
      const tokenAccount = getTokenAccount(tokenMint, user.publicKey)
      const balanceBefore = await user1Program.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(tokenAccount)
      )
      await unstake(user1Program, stakerId, nft)
      const balanceAfter = await user1Program.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(tokenAccount)
      )
      assert.equal(balanceBefore.value.uiAmount, balanceAfter.value.uiAmount, "expected to not claim any tokens")

      const tokenAcc = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      assert.equal(tokenAcc.amount, BigInt(1), "Expected to receive NFT back")
    })
  })

  describe("Collection with min-period enforced", () => {
    let nfts: DigitalAsset[]
    let collection: DigitalAsset

    before(async () => {
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
    })

    it("cannot add a collection with locked min period without UA", async () => {
      await expectFail(
        () =>
          initCollection(
            ownerProgram,
            stakerId,
            collection.publicKey,
            true,
            tokenMint,
            { mintToken: {} },
            10,
            60 * 60,
            null,
            60 * 60,
            true
          ),
        (err) => assertErrorCode(err, "UpdateAuthRequired")
      )
    })

    it("can add a new NFT collection, with a locked min period of an hour", async () => {
      const reward = 10
      const minimumPeriod = 60 * 60 * 1
      const duration = 1 * 60 * 60

      await updateV1(umi, {
        mint: collection.publicKey,
        newUpdateAuthority: owner.publicKey,
      }).sendAndConfirm(umi)

      const programConfig = await ownerProgram.account.programConfig.fetch(findProgramConfigPda())

      const usdcBalanceBefore = await ownerProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(getTokenAccount(USDC.publicKey, FEES_WALLET))
      )

      await initCollection(
        ownerProgram,
        stakerId,
        collection.publicKey,
        true,
        tokenMint,
        { mintToken: {} },
        reward,
        minimumPeriod,
        null,
        duration,
        true
      )
      const balanceAfter = await ownerProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(stakeVaultAccount)
      )

      const usdcBalanceAfter = await ownerProgram.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(getTokenAccount(USDC.publicKey, FEES_WALLET))
      )

      assert.equal(balanceAfter.value.uiAmount, balanceAfter.value.uiAmount, "expected no tokens to be transferred")
      assert.ok(
        new BN(usdcBalanceBefore.value.amount)
          .add(programConfig.extraCollectionFee)
          .eq(new BN(usdcBalanceAfter.value.amount)),
        "expected to pay for additional collection"
      )

      const stakooor = await ownerProgram.account.staker.fetch(stakerId)
      assert.ok(
        stakooor.collections.find(
          (coll) => coll.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey)
        ),
        "expected collection to be added"
      )

      assert.ok(
        stakooor.collections.find((c) => c.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey))
      )
    })

    it("can stake an NFT", async () => {
      const nft = nfts[0]
      await stake(user1Program, stakerId, nft)
      // wait for rewards to accrue
      await sleep(1000)
      const stakeRecord = await user1Program.account.stakeRecord.fetch(findStakeRecordPda(stakerId, nft.publicKey))
      assert.equal(stakeRecord.nftMint.toBase58(), nft.publicKey)
      assert.equal(stakeRecord.owner.toBase58(), user.publicKey)
      assert.ok(stakeRecord.stakedAt.toNumber() < Date.now() / 1000)

      const ownerToken = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      const custodyToken = await fetchToken(umi, getTokenAccount(nft.publicKey, nftAuthority))

      assert.equal(ownerToken.amount, BigInt(0), "expected holder to no longer hold NFT")
      assert.equal(custodyToken.amount, BigInt(1), "expected custody to hold NFT")
    })

    it("cannot claim if min period not exceeded", async () => {
      const nft = nfts[0]

      await expectFail(
        () => claim(user1Program, stakerId, nft),
        (err) => assert.equal(err.error.errorCode.code, "MinimumPeriodNotReached")
      )
    })

    it("cannot unstake if min period not exceeded", async () => {
      const nft = nfts[0]

      await expectFail(
        () => unstake(user1Program, stakerId, nft),
        (err) => assert.equal(err.error.errorCode.code, "MinimumPeriodNotReached")
      )
    })

    it("can unstake if stakooor collection closed, no more tokens have accrued", async () => {
      let stakooor = await ownerProgram.account.staker.fetch(stakerId)
      await closeCollection(ownerProgram, stakerId, collection.publicKey, stakooor.collections)
      const nft = nfts[0]
      const tokenAccount = getTokenAccount(tokenMint, user.publicKey)
      const balanceBefore = await user1Program.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(tokenAccount)
      )
      await unstake(user1Program, stakerId, nft)
      const balanceAfter = await user1Program.provider.connection.getTokenAccountBalance(
        toWeb3JsPublicKey(tokenAccount)
      )
      assert.ok(balanceBefore.value.uiAmount < balanceAfter.value.uiAmount, "expected tokens to be received")

      const tokenAcc = await fetchToken(umi, getTokenAccount(nft.publicKey, user.publicKey))
      assert.equal(tokenAcc.amount, BigInt(1), "Expected to receive NFT back")
      stakooor = await user1Program.account.staker.fetch(stakerId)

      assert.ok(
        !stakooor.collections.find(
          (coll) => coll.toBase58() === findStakooorCollectionId(stakerId, collection.publicKey)
        ),
        "expected collection to be removed"
      )
    })
  })

  describe("Collection with token vault", () => {
    let nfts: DigitalAsset[]
    let collection: DigitalAsset
    let tokenMint: PublicKey

    before(async () => {
      tokenMint = await createToken(umi, BigInt(10_000_000), 9, undefined, owner.publicKey)
      collection = await createCollection(umi)
      nfts = await mintNfts(collection.publicKey, 10, true, user.publicKey)
    })

    it("Cannot add a token vault collection without a duration", async () => {
      await expectFail(
        () =>
          initCollection(
            ownerProgram,
            stakerId,
            collection.publicKey,
            true,
            tokenMint,
            {
              transferToken: {},
            },
            10
          ),
        (err) => assertErrorCode(err, "DurationRequired")
      )
    })

    it("Can add a collection with a token vault", async () => {
      await initCollection(
        ownerProgram,
        stakerId,
        collection.publicKey,
        true,
        tokenMint,
        {
          transferToken: {},
        },
        10,
        0,
        null,
        3600
      )
      const stakooorCollectionId = findStakooorCollectionId(stakerId, collection.publicKey)
      const stakooor = await ownerProgram.account.staker.fetch(stakerId)
      const stakooorCollection = await ownerProgram.account.collection.fetch(stakooorCollectionId)

      assert.ok(
        stakooorCollection.currentBalance.eq(new BN(3600 * 10 * 10)),
        "Expected enough emissions for full staking period to be sent"
      )

      assert.ok(
        stakooor.collections.find((coll) => coll.toBase58() === stakooorCollectionId),
        "expected collection to be added"
      )
    })

    it("cannot increase the emission without first increasing the balance", async () => {
      await expectFail(
        () => changeReward(ownerProgram, 20, stakerId, collection.publicKey),
        (err) => assert.equal(err.error.errorCode.code, "InsufficientBalanceInVault")
      )
    })

    it("can increase the balance", async () => {
      await addFunds(ownerProgram, new BN(3600 * 10 * 10), stakerId, collection.publicKey)
      await expectFail(
        () => changeReward(ownerProgram, 30, stakerId, collection.publicKey),
        (err) => assert.equal(err.error.errorCode.code, "InsufficientBalanceInVault")
      )

      await changeReward(ownerProgram, 20, stakerId, collection.publicKey)

      const stakooorCollection = await ownerProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )

      assert.ok(stakooorCollection.currentBalance.eq(new BN(3600 * 10 * 20)), "expected balance have been updated")
    })

    it("can not increase the staking duration without increasing the balance", async () => {
      const stakooorCollection = await ownerProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      expectFail(
        () =>
          ownerProgram.methods
            .extendEmission(stakooorCollection.stakingEndsAt.add(new anchor.BN(60 * 60 * 24)))
            .accounts({
              staker: stakerId,
              collection: findStakooorCollectionId(stakerId, collection.publicKey),
            })
            .rpc(),
        (err) => assertErrorCode(err, "InsufficientBalanceInVault")
      )
    })

    it("can increase the staking if the balance is increased", async () => {
      const tokenBalance = await fetchToken(umi, getTokenAccount(tokenMint, owner.publicKey))

      const stakooorCollection = await ownerProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      const reward = stakooorCollection.reward.pop()
      const maxStakerCount = stakooorCollection.maxStakersCount
      const oneDay = new anchor.BN(60 * 60 * 24)
      const rewardPerDay = reward.mul(maxStakerCount).mul(oneDay)

      const extraTokensNeeded = Math.abs(Number(tokenBalance.amount) - rewardPerDay.toNumber())

      await mintTokensTo(umi, {
        mint: tokenMint,
        token: getTokenAccount(tokenMint, owner.publicKey),
        amount: tokenAmount(extraTokensNeeded, "token", 0).basisPoints,
      }).sendAndConfirm(umi)

      await addFunds(ownerProgram, rewardPerDay, stakerId, collection.publicKey)

      await expectFail(
        () =>
          ownerProgram.methods
            .extendEmission(stakooorCollection.stakingEndsAt.add(oneDay.mul(new anchor.BN(2))))
            .accounts({
              staker: stakerId,
              collection: findStakooorCollectionId(stakerId, collection.publicKey),
            })
            .rpc(),
        (err) => assertErrorCode(err, "InsufficientBalanceInVault")
      )

      await ownerProgram.methods
        .extendEmission(stakooorCollection.stakingEndsAt.add(oneDay))
        .accounts({
          staker: stakerId,
          collection: findStakooorCollectionId(stakerId, collection.publicKey),
        })
        .rpc()
    })

    it("only earns tokens for the time staked", async () => {
      const user2 = await createNewUser()
      const user2Program = programPaidBy(user2)
      const collection = await createCollection(umi)
      const user1Nft = await createNft(umi, true, collection.publicKey, user.publicKey)
      const user2Nft = await createNft(umi, true, collection.publicKey, user2.publicKey)
      const tokenMint = await createToken(umi, BigInt(10_000), 9, undefined, owner.publicKey)

      await initCollection(
        ownerProgram,
        stakerId,
        collection.publicKey,
        true,
        tokenMint,
        {
          transferToken: {},
        },
        10,
        0,
        null,
        2
      )
      await Promise.all([stake(user1Program, stakerId, user1Nft), stake(user2Program, stakerId, user2Nft)])
      await sleep(3_000)
      await claim(user1Program, stakerId, user1Nft)
      const user1BalAfterClaim = await fetchToken(umi, getTokenAccount(tokenMint, user.publicKey))

      assert.ok(user1BalAfterClaim.amount > BigInt(0), "Expected to have claimed some tokens")

      assert.ok(
        user1BalAfterClaim.amount < BigInt(30),
        "expected to receive less that 30 tokens, even if staked for 3 seconds"
      )

      await claim(user2Program, stakerId, user2Nft)
      const user2Bal = await fetchToken(umi, getTokenAccount(tokenMint, user2.publicKey))

      assert.equal(user1BalAfterClaim.amount, user2Bal.amount, "Expected both users to have the same amount to claim")
    })
  })

  describe("Points emission collection", () => {
    let collection: DigitalAsset
    let nft: DigitalAsset
    let nftRecordId: PublicKey
    let stakeRecordId: PublicKey

    before(async () => {
      collection = await createCollection(umi)
      nft = await createNft(umi, true, collection.publicKey, user.publicKey)
      nftRecordId = findNftRecordPda(stakerId, nft.publicKey)
      stakeRecordId = findStakeRecordPda(stakerId, nft.publicKey)
    })

    it("Can create a new points collection", async () => {
      await initCollection(ownerProgram, stakerId, collection.publicKey, false, null, { points: {} }, 1)
      const stakooorCollection = await ownerProgram.account.collection.fetch(
        findStakooorCollectionId(stakerId, collection.publicKey)
      )
      assert.ok(stakooorCollection.rewardToken === null, "Expected no reward token to be set")
      assert.ok(stakooorCollection.reward.pop().eq(new anchor.BN(1)), "Expected reward to be 1")
    })

    it("can stake an NFT creating a new nft record and stake record account", async () => {
      await stake(user1Program, stakerId, nft)

      const stakeRecord = await user1Program.account.stakeRecord.fetch(stakeRecordId)
      assert.equal(stakeRecord.nftMint.toBase58(), nft.publicKey, "Expected nft mint to be added to stake record")

      const nftRecord = await user1Program.account.nftRecord.fetch(nftRecordId)
      assert.equal(nftRecord.nftMint.toBase58(), nft.publicKey, "Expected nft mint to be added to nft record")
      assert.equal(nftRecord.points.toNumber(), 0, "expected  points to start at 0")
    })

    it("can unstake, closing the stake record and persisting the NFT record", async () => {
      const stakeRecord = await user1Program.account.stakeRecord.fetch(stakeRecordId)

      await sleep(1000)
      const tx = await unstake(user1Program, stakerId, nft)
      const slot = await umi.rpc.getSlot({ id: tx })
      const blockTime = await umi.rpc.getBlockTime(slot)

      const timeStakedFor = Number(blockTime - BigInt(stakeRecord.stakedAt.toNumber()))

      const stakeRecordExists = await umi.rpc.accountExists(stakeRecordId)
      assert.ok(!stakeRecordExists, "Expected stake record to have been closed")

      const nftRecordExists = await umi.rpc.accountExists(nftRecordId)
      assert.ok(nftRecordExists, "Expected nft record to still be open")

      const nftRecord = await user1Program.account.nftRecord.fetch(nftRecordId)
      assert.equal(nftRecord.nftMint.toBase58(), nft.publicKey)
      assert.equal(nftRecord.points.toNumber(), timeStakedFor, "Expected 1 point per second to have been accrued")
    })

    it("can be staked and unstaked by another user, adding to the points tally", async () => {
      const nftRecordBefore = await user2Program.account.nftRecord.fetch(nftRecordId)
      await transferV1(umi, {
        mint: nft.publicKey,
        token: getTokenAccount(nft.publicKey, user.publicKey),
        tokenOwner: user.publicKey,
        destinationToken: getTokenAccount(nft.publicKey, user2.publicKey),
        destinationOwner: user2.publicKey,
        authority: createSignerFromKeypair(umi, user),
        tokenStandard: TokenStandard.ProgrammableNonFungible,
      }).sendAndConfirm(umi)

      const stakeTx = await stake(user2Program, stakerId, nft)
      const stakedAt = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: stakeTx }))
      await sleep(1000)

      const unstakeTx = await unstake(user2Program, stakerId, nft)
      const unstakedAt = await umi.rpc.getBlockTime(await umi.rpc.getSlot({ id: unstakeTx }))

      const stakedFor = Number(unstakedAt - stakedAt)
      const nftRecordAfter = await user2Program.account.nftRecord.fetch(nftRecordId)

      assert.equal(
        nftRecordAfter.points.toNumber() - nftRecordBefore.points.toNumber(),
        stakedFor,
        "Expected points to have increased by time staked"
      )
    })
  })
})
