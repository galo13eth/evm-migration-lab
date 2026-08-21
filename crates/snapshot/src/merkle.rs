use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use alloy::primitives::{Address, B256, U256, keccak256};
use alloy::sol;
use alloy::sol_types::SolValue;
use merkrs::{SimpleMerkleTree, simple};

use crate::model::{
    Campaign, Manifest, ManifestEntry, OwnerMultiProof, ProofBundle, SingleProof, address_hex,
    b256_hex,
};
use crate::{Result, SnapshotError};

sol! {
    struct MigrationLeaf {
        bytes32 migrationId;
        uint256 sourceChainId;
        address sourceContract;
        uint256 snapshotBlock;
        bytes32 sourceBlockHash;
        uint256 destinationChainId;
        uint8 standard;
        uint256 tokenId;
        uint256 amount;
        address sourceOwner;
        address claimAuthority;
        address destinationRecipient;
        uint256 leafIndex;
    }
}

pub struct MerkleArtifacts {
    pub manifest: Manifest,
    pub proofs: ProofBundle,
}

pub fn build(mut manifest: Manifest) -> Result<MerkleArtifacts> {
    if manifest.entries.is_empty() {
        return Err(SnapshotError::Merkle("manifest has no entries".into()));
    }

    let inner_hashes = manifest
        .entries
        .iter()
        .map(|entry| inner_hash(&manifest.campaign, entry))
        .collect::<Result<Vec<_>>>()?;
    let tree = SimpleMerkleTree::new(&inner_hashes, simple::Options::default())
        .map_err(|error| SnapshotError::Merkle(error.to_string()))?;
    let root = B256::from(*tree.root());

    let mut leaf_to_index = HashMap::<[u8; 32], u64>::new();
    let mut single_proofs = Vec::with_capacity(manifest.entries.len());
    for (index, entry) in manifest.entries.iter_mut().enumerate() {
        let leaf = keccak256(inner_hashes[index]);
        entry.leaf_hash = b256_hex(leaf);
        leaf_to_index.insert(leaf.0, entry.leaf_index);
        let proof = tree
            .proof_by_index(index)
            .map_err(|error| SnapshotError::Merkle(error.to_string()))?;
        single_proofs.push(SingleProof {
            leaf_index: entry.leaf_index,
            proof: proof
                .into_iter()
                .map(|hash| b256_hex(B256::from(hash)))
                .collect(),
        });
    }

    let mut owner_groups = BTreeMap::<String, Vec<usize>>::new();
    for (index, entry) in manifest.entries.iter().enumerate() {
        owner_groups
            .entry(entry.claim_authority.clone())
            .or_default()
            .push(index);
    }
    let owner_multi_proofs = owner_groups
        .into_iter()
        .map(|(claim_authority, indices)| {
            let proof = tree
                .multi_proof_by_indices(&indices)
                .map_err(|error| SnapshotError::Merkle(error.to_string()))?;
            let leaf_indices = proof
                .leaves
                .iter()
                .map(|leaf| {
                    leaf_to_index.get(leaf).copied().ok_or_else(|| {
                        SnapshotError::Merkle("multiproof returned an unknown leaf".into())
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(OwnerMultiProof {
                claim_authority,
                leaf_indices,
                proof: proof
                    .proof
                    .into_iter()
                    .map(|hash| b256_hex(B256::from(hash)))
                    .collect(),
                proof_flags: proof.proof_flags,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(MerkleArtifacts {
        manifest,
        proofs: ProofBundle {
            root: b256_hex(root),
            single_proofs,
            owner_multi_proofs,
        },
    })
}

pub fn inner_hash(campaign: &Campaign, entry: &ManifestEntry) -> Result<[u8; 32]> {
    let value = MigrationLeaf {
        migrationId: B256::from_str(&campaign.migration_id)
            .map_err(|error| SnapshotError::Config(format!("invalid migration id: {error}")))?,
        sourceChainId: U256::from(campaign.source_chain_id),
        sourceContract: Address::from_str(&campaign.source_contract)
            .map_err(|error| SnapshotError::Config(format!("invalid source contract: {error}")))?,
        snapshotBlock: U256::from(campaign.snapshot_block),
        sourceBlockHash: B256::from_str(&campaign.snapshot_block_hash).map_err(|error| {
            SnapshotError::Config(format!("invalid snapshot block hash: {error}"))
        })?,
        destinationChainId: U256::from(campaign.destination_chain_id),
        standard: entry.standard,
        tokenId: entry.token_id_u256()?,
        amount: entry.amount_u256()?,
        sourceOwner: entry.source_owner_address()?,
        claimAuthority: entry.claim_authority_address()?,
        destinationRecipient: entry.recipient_address()?,
        leafIndex: U256::from(entry.leaf_index),
    };
    Ok(keccak256(value.abi_encode()).0)
}

#[allow(clippy::too_many_arguments)] // Mirrors the eight immutable campaign fields.
pub fn campaign(
    migration_id: B256,
    source_chain_id: u64,
    source_contract: Address,
    snapshot_block: u64,
    snapshot_block_hash: B256,
    destination_chain_id: u64,
    standard: u8,
    finality_policy: String,
) -> Campaign {
    Campaign {
        migration_id: b256_hex(migration_id),
        source_chain_id,
        source_contract: address_hex(source_contract),
        snapshot_block,
        snapshot_block_hash: b256_hex(snapshot_block_hash),
        destination_chain_id,
        standard,
        finality_policy,
        leaf_encoding: [
            "bytes32", "uint256", "address", "uint256", "bytes32", "uint256", "uint8", "uint256",
            "uint256", "address", "address", "address", "uint256",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{address, b256};

    use super::*;

    #[test]
    fn builds_deterministic_proofs() {
        let campaign = campaign(
            b256!("1111111111111111111111111111111111111111111111111111111111111111"),
            1,
            address!("0000000000000000000000000000000000000001"),
            10,
            b256!("2222222222222222222222222222222222222222222222222222222222222222"),
            84532,
            1,
            "confirmations:12".into(),
        );
        let entries = (0..4)
            .map(|index| ManifestEntry {
                standard: 1,
                token_id: (index + 1).to_string(),
                amount: "1".into(),
                source_owner: "0x0000000000000000000000000000000000000002".into(),
                claim_authority: "0x0000000000000000000000000000000000000002".into(),
                destination_recipient: "0x0000000000000000000000000000000000000002".into(),
                leaf_index: index,
                leaf_hash: String::new(),
            })
            .collect();
        let manifest = Manifest {
            format: "evm-migration-manifest-v2".into(),
            campaign,
            entries,
        };
        let first = build(manifest.clone()).unwrap();
        let second = build(manifest).unwrap();
        assert_eq!(first.manifest, second.manifest);
        assert_eq!(first.proofs, second.proofs);
        assert_eq!(first.proofs.single_proofs.len(), 4);
        assert_eq!(first.proofs.owner_multi_proofs.len(), 1);
    }
}
