use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use alloy::primitives::{Address, B256, U256};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{Result, SnapshotError};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum TokenStandard {
    Erc721,
    Erc1155,
}

impl TokenStandard {
    pub const fn code(self) -> u8 {
        match self {
            Self::Erc721 => 1,
            Self::Erc1155 => 2,
        }
    }
}

impl fmt::Display for TokenStandard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Erc721 => f.write_str("erc721"),
            Self::Erc1155 => f.write_str("erc1155"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPosition {
    pub block_number: u64,
    pub transaction_index: u64,
    pub log_index: u64,
    pub sub_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub position: EventPosition,
    pub from: Address,
    pub to: Address,
    pub token_id: U256,
    pub amount: U256,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Holding {
    pub owner: Address,
    pub token_id: U256,
    pub amount: U256,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointConfig {
    pub migration_id: B256,
    pub source_chain_id: u64,
    pub source_contract: Address,
    pub standard: TokenStandard,
    pub from_block: u64,
    pub snapshot_block: u64,
    pub snapshot_block_hash: B256,
    pub chunk_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub format: String,
    pub config: CheckpointConfig,
    pub chunks: BTreeMap<u64, Vec<Transfer>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub migration_id: String,
    pub source_chain_id: u64,
    pub source_contract: String,
    pub snapshot_block: u64,
    pub snapshot_block_hash: String,
    pub standard: u8,
    pub leaf_encoding: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub standard: u8,
    pub token_id: String,
    pub amount: String,
    pub source_owner: String,
    pub destination_recipient: String,
    pub leaf_index: u64,
    pub leaf_hash: String,
}

impl ManifestEntry {
    pub fn token_id_u256(&self) -> Result<U256> {
        U256::from_str(&self.token_id)
            .map_err(|error| SnapshotError::Config(format!("invalid token id: {error}")))
    }

    pub fn amount_u256(&self) -> Result<U256> {
        U256::from_str(&self.amount)
            .map_err(|error| SnapshotError::Config(format!("invalid amount: {error}")))
    }

    pub fn source_owner_address(&self) -> Result<Address> {
        Address::from_str(&self.source_owner)
            .map_err(|error| SnapshotError::Config(format!("invalid source owner: {error}")))
    }

    pub fn recipient_address(&self) -> Result<Address> {
        Address::from_str(&self.destination_recipient)
            .map_err(|error| SnapshotError::Config(format!("invalid recipient: {error}")))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub campaign: Campaign,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleProof {
    pub leaf_index: u64,
    pub proof: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerMultiProof {
    pub source_owner: String,
    pub leaf_indices: Vec<u64>,
    pub proof: Vec<String>,
    pub proof_flags: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofBundle {
    pub root: String,
    pub single_proofs: Vec<SingleProof>,
    pub owner_multi_proofs: Vec<OwnerMultiProof>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationSample {
    pub leaf_index: u64,
    pub expected: String,
    pub actual: String,
    pub consistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    pub snapshot_block: u64,
    pub snapshot_block_hash: String,
    pub samples: Vec<ReconciliationSample>,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusArtifact {
    pub snapshot_block: u64,
    pub manifest_entries: usize,
    pub merkle_root: String,
    pub claims_completed: u64,
    pub reconciliation_status: String,
    pub last_verified_commit: String,
}

pub fn address_hex(address: Address) -> String {
    format!("{address:#x}")
}

pub fn b256_hex(value: B256) -> String {
    format!("{value:#x}")
}
