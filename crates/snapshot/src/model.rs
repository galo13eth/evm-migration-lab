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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum FinalityPolicy {
    Confirmations,
    SafeBlockTag,
    FinalizedBlockTag,
    ManualReviewed,
    L2Finalized,
}

impl fmt::Display for FinalityPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Confirmations => f.write_str("confirmations"),
            Self::SafeBlockTag => f.write_str("safe-block-tag"),
            Self::FinalizedBlockTag => f.write_str("finalized-block-tag"),
            Self::ManualReviewed => f.write_str("manual-reviewed"),
            Self::L2Finalized => f.write_str("l2-finalized"),
        }
    }
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
    pub finality_policy: FinalityPolicy,
    pub confirmations: u64,
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
    #[serde(with = "decimal_u64")]
    pub source_chain_id: u64,
    pub source_contract: String,
    #[serde(with = "decimal_u64")]
    pub snapshot_block: u64,
    pub snapshot_block_hash: String,
    #[serde(with = "decimal_u64")]
    pub destination_chain_id: u64,
    pub standard: u8,
    pub finality_policy: String,
    pub leaf_encoding: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub standard: u8,
    pub token_id: String,
    pub amount: String,
    pub source_owner: String,
    pub claim_authority: String,
    pub destination_recipient: String,
    #[serde(with = "decimal_u64")]
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

    pub fn claim_authority_address(&self) -> Result<Address> {
        Address::from_str(&self.claim_authority)
            .map_err(|error| SnapshotError::Config(format!("invalid claim authority: {error}")))
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
    #[serde(with = "decimal_u64")]
    pub leaf_index: u64,
    pub proof: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerMultiProof {
    pub claim_authority: String,
    #[serde(with = "decimal_u64_vec")]
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
    #[serde(with = "decimal_u64")]
    pub leaf_index: u64,
    pub expected: String,
    pub actual: String,
    pub consistent: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    #[serde(with = "decimal_u64")]
    pub snapshot_block: u64,
    pub snapshot_block_hash: String,
    pub samples: Vec<ReconciliationSample>,
    pub status: String,
    #[serde(with = "decimal_usize")]
    pub sampled_entries: usize,
    #[serde(with = "decimal_usize")]
    pub manifest_entries: usize,
    #[serde(with = "decimal_f64")]
    pub sample_coverage: f64,
    pub sample_seed: String,
    pub providers_compared: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusArtifact {
    pub environment: String,
    #[serde(with = "decimal_u64")]
    pub chain_id: u64,
    pub live: bool,
    #[serde(with = "decimal_u64")]
    pub generated_at: u64,
    #[serde(with = "decimal_u64")]
    pub snapshot_block: u64,
    pub snapshot_block_hash: String,
    #[serde(with = "decimal_usize")]
    pub manifest_entries: usize,
    pub merkle_root: String,
    #[serde(with = "decimal_u64")]
    pub claims_completed: u64,
    pub reconciliation_status: String,
    pub last_verified_commit: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDigests {
    pub format: String,
    pub files: BTreeMap<String, String>,
    pub bundle_digest: String,
    pub cli_version: String,
    pub verified_commit: String,
    #[serde(with = "decimal_u64")]
    pub source_block: u64,
    pub source_block_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentBundle {
    pub format: String,
    pub merkle_root: String,
    pub path: String,
}

pub fn address_hex(address: Address) -> String {
    format!("{address:#x}")
}

pub fn b256_hex(value: B256) -> String {
    format!("{value:#x}")
}

mod decimal_u64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

mod decimal_usize {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &usize, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<usize, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

mod decimal_u64_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(values: &[u64], serializer: S) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u64>, D::Error> {
        Vec::<String>::deserialize(deserializer)?
            .into_iter()
            .map(|value| value.parse().map_err(serde::de::Error::custom))
            .collect()
    }
}

mod decimal_f64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{value:.8}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
