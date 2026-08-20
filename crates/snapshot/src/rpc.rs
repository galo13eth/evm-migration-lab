use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, U256, keccak256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, Log};
use alloy::sol_types::SolEvent;
use clap::Parser;
use futures::{StreamExt, stream};
use tracing::{info, warn};

use crate::artifacts;
use crate::merkle;
use crate::model::{
    Checkpoint, CheckpointConfig, EventPosition, Manifest, ManifestEntry, Reconciliation,
    ReconciliationSample, TokenStandard, Transfer, address_hex, b256_hex,
};
use crate::reconstruct;
use crate::{Result, SnapshotError};

mod erc721 {
    use alloy::sol;

    sol! {
        event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);

        #[sol(rpc)]
        interface SnapshotToken {
            function ownerOf(uint256 tokenId) external view returns (address);
        }
    }
}

mod erc1155 {
    use alloy::sol;

    sol! {
        event TransferSingle(
            address indexed operator,
            address indexed from,
            address indexed to,
            uint256 id,
            uint256 value
        );
        event TransferBatch(
            address indexed operator,
            address indexed from,
            address indexed to,
            uint256[] ids,
            uint256[] values
        );

        #[sol(rpc)]
        interface SnapshotToken {
            function balanceOf(address owner, uint256 id) external view returns (uint256);
        }
    }
}

#[derive(Clone, Debug, Parser)]
#[command(
    name = "evm-snapshot",
    about = "Reconstruct ERC token state and generate claims"
)]
pub struct Args {
    #[arg(long, env = "SOURCE_RPC_URL")]
    pub rpc_url: String,
    #[arg(long)]
    pub contract: Address,
    #[arg(long, value_enum)]
    pub standard: TokenStandard,
    #[arg(long)]
    pub snapshot_block: u64,
    #[arg(long, default_value_t = 0)]
    pub from_block: u64,
    #[arg(long)]
    pub migration_id: B256,
    #[arg(long, default_value = "output")]
    pub output: PathBuf,
    #[arg(long)]
    pub checkpoint: Option<PathBuf>,
    #[arg(long, default_value_t = 2_000)]
    pub chunk_size: u64,
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,
    #[arg(long, default_value_t = 10)]
    pub sample_size: usize,
    #[arg(long, default_value_t = 12)]
    pub confirmations: u64,
    #[arg(long)]
    pub recipient_map: Option<PathBuf>,
    #[arg(long, env = "GITHUB_SHA", default_value = "local")]
    pub verified_commit: String,
}

pub async fn run(args: Args) -> Result<()> {
    validate_args(&args)?;
    let provider = ProviderBuilder::new()
        .connect(&args.rpc_url)
        .await
        .map_err(|error| SnapshotError::Rpc(error.to_string()))?;
    let source_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|error| SnapshotError::Rpc(error.to_string()))?;
    let latest = provider
        .get_block_number()
        .await
        .map_err(|error| SnapshotError::Rpc(error.to_string()))?;
    if latest < args.snapshot_block.saturating_add(args.confirmations) {
        return Err(SnapshotError::Config(format!(
            "snapshot block {} has {} confirmations; {} required",
            args.snapshot_block,
            latest.saturating_sub(args.snapshot_block),
            args.confirmations
        )));
    }
    let boundary_hash = block_hash(&provider, args.snapshot_block).await?;
    let config = CheckpointConfig {
        migration_id: args.migration_id,
        source_chain_id,
        source_contract: args.contract,
        standard: args.standard,
        from_block: args.from_block,
        snapshot_block: args.snapshot_block,
        snapshot_block_hash: boundary_hash,
        chunk_size: args.chunk_size,
    };
    let checkpoint_path = args
        .checkpoint
        .clone()
        .unwrap_or_else(|| args.output.join(".snapshot-checkpoint.json"));
    let mut checkpoint = load_checkpoint(&checkpoint_path, &config)?;
    fetch_missing_chunks(&provider, &args, &checkpoint_path, &mut checkpoint).await?;

    let final_hash = block_hash(&provider, args.snapshot_block).await?;
    if final_hash != boundary_hash {
        return Err(SnapshotError::BoundaryChanged {
            expected: b256_hex(boundary_hash),
            actual: b256_hex(final_hash),
        });
    }

    let transfers = checkpoint
        .chunks
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let holdings = reconstruct::reconstruct(args.standard, &transfers)?;
    let recipients = load_recipients(args.recipient_map.as_deref())?;
    let entries = holdings
        .into_iter()
        .enumerate()
        .map(|(index, holding)| {
            let owner = address_hex(holding.owner);
            let recipient = recipients
                .get(&owner)
                .cloned()
                .unwrap_or_else(|| owner.clone());
            Ok(ManifestEntry {
                standard: args.standard.code(),
                token_id: holding.token_id.to_string(),
                amount: holding.amount.to_string(),
                source_owner: owner,
                destination_recipient: address_hex(parse_address(&recipient)?),
                leaf_index: index as u64,
                leaf_hash: String::new(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = Manifest {
        format: "evm-migration-manifest-v1".into(),
        campaign: merkle::campaign(
            args.migration_id,
            source_chain_id,
            args.contract,
            args.snapshot_block,
            boundary_hash,
            args.standard.code(),
        ),
        entries,
    };

    let reconciliation = reconcile(&provider, &args, boundary_hash, &manifest.entries).await?;
    artifacts::write_reconciliation(&args.output, &reconciliation)?;
    if reconciliation.status != "consistent" {
        return Err(SnapshotError::Reconciliation);
    }
    let built = merkle::build(manifest)?;
    artifacts::write_success(
        &args.output,
        &built.manifest,
        &built.proofs,
        &reconciliation,
        &args.verified_commit,
    )?;
    if checkpoint_path.exists() {
        fs::remove_file(&checkpoint_path).map_err(|source| SnapshotError::Io {
            path: checkpoint_path,
            source,
        })?;
    }
    info!(entries = built.manifest.entries.len(), root = %built.proofs.root, "snapshot complete");
    Ok(())
}

fn validate_args(args: &Args) -> Result<()> {
    if args.chunk_size == 0 {
        return Err(SnapshotError::Config(
            "chunk size must be greater than zero".into(),
        ));
    }
    if args.concurrency == 0 {
        return Err(SnapshotError::Config(
            "concurrency must be greater than zero".into(),
        ));
    }
    if args.from_block > args.snapshot_block {
        return Err(SnapshotError::Config(
            "from block is after snapshot block".into(),
        ));
    }
    Ok(())
}

async fn block_hash<P: Provider>(provider: &P, block_number: u64) -> Result<B256> {
    provider
        .get_block_by_number(block_number.into())
        .await
        .map_err(|error| SnapshotError::Rpc(error.to_string()))?
        .map(|block| block.header.hash)
        .ok_or(SnapshotError::MissingBlock(block_number))
}

fn load_checkpoint(path: &Path, config: &CheckpointConfig) -> Result<Checkpoint> {
    if !path.exists() {
        return Ok(Checkpoint {
            format: "evm-migration-checkpoint-v1".into(),
            config: config.clone(),
            chunks: BTreeMap::new(),
        });
    }
    let checkpoint: Checkpoint = artifacts::read_json(path)?;
    if checkpoint.config != *config {
        if checkpoint.config.snapshot_block_hash != config.snapshot_block_hash {
            return Err(SnapshotError::BoundaryChanged {
                expected: b256_hex(checkpoint.config.snapshot_block_hash),
                actual: b256_hex(config.snapshot_block_hash),
            });
        }
        return Err(SnapshotError::CheckpointMismatch(format!(
            "stored config {:?}, requested config {:?}",
            checkpoint.config, config
        )));
    }
    info!(chunks = checkpoint.chunks.len(), "resuming checkpoint");
    Ok(checkpoint)
}

async fn fetch_missing_chunks<P: Provider + Clone>(
    provider: &P,
    args: &Args,
    checkpoint_path: &Path,
    checkpoint: &mut Checkpoint,
) -> Result<()> {
    let mut ranges = Vec::new();
    let mut start = args.from_block;
    loop {
        let end = start
            .saturating_add(args.chunk_size - 1)
            .min(args.snapshot_block);
        if !checkpoint.chunks.contains_key(&start) {
            ranges.push((start, end));
        }
        if end == args.snapshot_block {
            break;
        }
        start = end + 1;
    }

    let jobs = stream::iter(ranges).map(|(start, end)| {
        let provider = provider.clone();
        async move {
            let filter = Filter::new()
                .address(args.contract)
                .from_block(start)
                .to_block(end);
            let logs = provider
                .get_logs(&filter)
                .await
                .map_err(|error| SnapshotError::Rpc(error.to_string()))?;
            let transfers = decode_logs(args.standard, logs)?;
            Ok::<_, SnapshotError>((start, end, transfers))
        }
    });
    let mut jobs = jobs.buffer_unordered(args.concurrency);
    while let Some(result) = jobs.next().await {
        let (start, end, transfers) = result?;
        checkpoint.chunks.insert(start, transfers);
        if let Some(parent) = checkpoint_path.parent() {
            fs::create_dir_all(parent).map_err(|source| SnapshotError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        artifacts::write_json(checkpoint_path, checkpoint)?;
        info!(start, end, "checkpointed log chunk");
    }
    Ok(())
}

fn decode_logs(standard: TokenStandard, logs: Vec<Log>) -> Result<Vec<Transfer>> {
    let mut transfers = Vec::new();
    for log in logs {
        if log.removed {
            return Err(SnapshotError::MalformedLog(
                "RPC returned a removed log".into(),
            ));
        }
        let position = EventPosition {
            block_number: log
                .block_number
                .ok_or_else(|| missing_position("block number"))?,
            transaction_index: log
                .transaction_index
                .ok_or_else(|| missing_position("transaction index"))?,
            log_index: log.log_index.ok_or_else(|| missing_position("log index"))?,
            sub_index: 0,
        };
        match standard {
            TokenStandard::Erc721 if log.topic0() == Some(&erc721::Transfer::SIGNATURE_HASH) => {
                let event = log
                    .log_decode::<erc721::Transfer>()
                    .map_err(|error| SnapshotError::MalformedLog(error.to_string()))?
                    .inner
                    .data;
                transfers.push(Transfer {
                    position,
                    from: event.from,
                    to: event.to,
                    token_id: event.tokenId,
                    amount: U256::from(1),
                });
            }
            TokenStandard::Erc1155
                if log.topic0() == Some(&erc1155::TransferSingle::SIGNATURE_HASH) =>
            {
                let event = log
                    .log_decode::<erc1155::TransferSingle>()
                    .map_err(|error| SnapshotError::MalformedLog(error.to_string()))?
                    .inner
                    .data;
                transfers.push(Transfer {
                    position,
                    from: event.from,
                    to: event.to,
                    token_id: event.id,
                    amount: event.value,
                });
            }
            TokenStandard::Erc1155
                if log.topic0() == Some(&erc1155::TransferBatch::SIGNATURE_HASH) =>
            {
                let event = log
                    .log_decode::<erc1155::TransferBatch>()
                    .map_err(|error| SnapshotError::MalformedLog(error.to_string()))?
                    .inner
                    .data;
                if event.ids.len() != event.values.len() {
                    return Err(SnapshotError::MalformedLog(
                        "TransferBatch ids and values lengths differ".into(),
                    ));
                }
                for (index, (token_id, amount)) in
                    event.ids.into_iter().zip(event.values).enumerate()
                {
                    let mut item_position = position.clone();
                    item_position.sub_index = u32::try_from(index).map_err(|_| {
                        SnapshotError::MalformedLog("TransferBatch has too many items".into())
                    })?;
                    transfers.push(Transfer {
                        position: item_position,
                        from: event.from,
                        to: event.to,
                        token_id,
                        amount,
                    });
                }
            }
            _ => warn!(signature = ?log.topic0(), "ignored unrelated contract log"),
        }
    }
    Ok(transfers)
}

fn missing_position(field: &str) -> SnapshotError {
    SnapshotError::MalformedLog(format!("log is missing {field}"))
}

fn load_recipients(path: Option<&Path>) -> Result<BTreeMap<String, String>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let raw: BTreeMap<String, String> = artifacts::read_json(path)?;
    raw.into_iter()
        .map(|(owner, recipient)| {
            Ok((
                address_hex(parse_address(&owner)?),
                address_hex(parse_address(&recipient)?),
            ))
        })
        .collect()
}

fn parse_address(value: &str) -> Result<Address> {
    value
        .parse()
        .map_err(|error| SnapshotError::Config(format!("invalid address {value}: {error}")))
}

async fn reconcile<P: Provider + Clone>(
    provider: &P,
    args: &Args,
    boundary_hash: B256,
    entries: &[ManifestEntry],
) -> Result<Reconciliation> {
    let indices = sample_indices(entries, args.sample_size, boundary_hash);
    let mut samples = Vec::with_capacity(indices.len());
    match args.standard {
        TokenStandard::Erc721 => {
            let token = erc721::SnapshotToken::new(args.contract, provider.clone());
            for index in indices {
                let entry = &entries[index];
                let actual = token
                    .ownerOf(entry.token_id_u256()?)
                    .block(BlockId::from(args.snapshot_block))
                    .call()
                    .await
                    .map_err(|error| SnapshotError::Rpc(error.to_string()))?;
                let actual = address_hex(actual);
                samples.push(ReconciliationSample {
                    leaf_index: entry.leaf_index,
                    expected: entry.source_owner.clone(),
                    consistent: actual == entry.source_owner,
                    actual,
                });
            }
        }
        TokenStandard::Erc1155 => {
            let token = erc1155::SnapshotToken::new(args.contract, provider.clone());
            for index in indices {
                let entry = &entries[index];
                let actual = token
                    .balanceOf(entry.source_owner_address()?, entry.token_id_u256()?)
                    .block(BlockId::from(args.snapshot_block))
                    .call()
                    .await
                    .map_err(|error| SnapshotError::Rpc(error.to_string()))?
                    .to_string();
                samples.push(ReconciliationSample {
                    leaf_index: entry.leaf_index,
                    expected: entry.amount.clone(),
                    consistent: actual == entry.amount,
                    actual,
                });
            }
        }
    }
    let status = if samples.iter().all(|sample| sample.consistent) {
        "consistent"
    } else {
        "inconsistent"
    };
    Ok(Reconciliation {
        snapshot_block: args.snapshot_block,
        snapshot_block_hash: b256_hex(boundary_hash),
        samples,
        status: status.into(),
    })
}

fn sample_indices(entries: &[ManifestEntry], sample_size: usize, seed: B256) -> Vec<usize> {
    let mut scored = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let mut bytes = Vec::with_capacity(40);
            bytes.extend_from_slice(seed.as_slice());
            bytes.extend_from_slice(&entry.leaf_index.to_be_bytes());
            (keccak256(bytes), index)
        })
        .collect::<Vec<_>>();
    scored.sort_unstable_by_key(|(score, _)| *score);
    scored.truncate(sample_size.min(scored.len()));
    scored.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use alloy::primitives::b256;

    use super::*;

    #[test]
    fn sample_selection_is_deterministic() {
        let entries = (0..20)
            .map(|leaf_index| ManifestEntry {
                standard: 1,
                token_id: leaf_index.to_string(),
                amount: "1".into(),
                source_owner: address_hex(Address::ZERO),
                destination_recipient: address_hex(Address::ZERO),
                leaf_index,
                leaf_hash: String::new(),
            })
            .collect::<Vec<_>>();
        let seed = b256!("1111111111111111111111111111111111111111111111111111111111111111");
        assert_eq!(
            sample_indices(&entries, 5, seed),
            sample_indices(&entries, 5, seed)
        );
        assert_eq!(sample_indices(&entries, 100, seed).len(), 20);
    }
}
