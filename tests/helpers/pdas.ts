import { umi } from "./umi"
import idl from "../../target/idl/stake.json"
import { PublicKey, publicKey } from "@metaplex-foundation/umi"
import { string, publicKey as publicKeySerializer } from "@metaplex-foundation/umi-serializers"
import { findAssociatedTokenPda } from "@metaplex-foundation/mpl-toolbox"
import { findMasterEditionPda, findMetadataPda, findTokenRecordPda } from "@metaplex-foundation/mpl-token-metadata"

const programId = publicKey(idl.metadata.address)

export function findProgramConfigPda() {
  return umi.eddsa.findPda(programId, [string({ size: "variable" }).serialize("program-config")])[0]
}

export function findProgramDataAddress() {
  return umi.eddsa.findPda(publicKey("BPFLoaderUpgradeab1e11111111111111111111111"), [
    publicKeySerializer().serialize(programId),
  ])[0]
}

export function findStakooorCollectionId(staker: PublicKey, collection: PublicKey) {
  return umi.eddsa.findPda(programId, [
    string({ size: "variable" }).serialize("STAKE"),
    publicKeySerializer().serialize(staker),
    publicKeySerializer().serialize(collection),
    string({ size: "variable" }).serialize("collection"),
  ])[0]
}

export function findTokenAuthorityPda(staker: PublicKey) {
  return umi.eddsa.findPda(programId, [
    string({ size: "variable" }).serialize("STAKE"),
    publicKeySerializer().serialize(staker),
    string({ size: "variable" }).serialize("token-authority"),
  ])[0]
}

export function findNftAuthorityPda(staker: PublicKey) {
  return umi.eddsa.findPda(programId, [
    string({ size: "variable" }).serialize("STAKE"),
    publicKeySerializer().serialize(staker),
    string({ size: "variable" }).serialize("nft-authority"),
  ])[0]
}

export function findNftRecordPda(staker: PublicKey, nftMint: PublicKey) {
  return umi.eddsa.findPda(programId, [
    string({ size: "variable" }).serialize("STAKE"),
    publicKeySerializer().serialize(staker),
    publicKeySerializer().serialize(nftMint),
    string({ size: "variable" }).serialize("nft-record"),
  ])[0]
}

export function findStakeRecordPda(staker: PublicKey, nftMint: PublicKey) {
  return umi.eddsa.findPda(programId, [
    string({ size: "variable" }).serialize("STAKE"),
    publicKeySerializer().serialize(staker),
    publicKeySerializer().serialize(nftMint),
    string({ size: "variable" }).serialize("stake-record"),
  ])[0]
}

export function findNftMetadataPda(nftMint: PublicKey) {
  return findMetadataPda(umi, { mint: nftMint })[0]
}

export function findNftMasterEditionPda(nftMint: PublicKey) {
  return findMasterEditionPda(umi, { mint: nftMint })[0]
}

export function getTokenAccount(mint: PublicKey, owner: PublicKey) {
  return findAssociatedTokenPda(umi, {
    owner,
    mint,
  })[0]
}

export function getTokenRecordPda(mint: PublicKey, owner: PublicKey) {
  return findTokenRecordPda(umi, {
    mint,
    token: getTokenAccount(mint, owner),
  })[0]
}
