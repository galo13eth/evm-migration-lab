use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::primitives::{Address, B256, U256, keccak256};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, Log};
use alloy::sol_types::SolEvent;
use clap::Parser;
use futures::{StreamExt, stream};
use tracing::{info, warn};

use crate::error::{rpc_error, rpc_message};
use crate::merkle;
use crate::model::{
    Checkpoint, CheckpointConfig, EventPosition, FinalityPolicy, Manifest, ManifestEntry,
    Reconciliation, ReconciliationSample, TokenStandard, Transfer, address_hex, b256_hex,
};
use crate::reconstruct;
use crate::{Result, SnapshotError};
use crate::{artifacts, authorization};

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
    #[arg(long)]
    pub destination_chain_id: u64,
    #[arg(long, default_value = "output")]
    pub output: PathBuf,
    #[arg(long)]
    pub checkpoint: Option<PathBuf>,
    #[arg(long, default_value_t = 2_000)]
    pub chunk_size: u64,
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,
    #[arg(long, default_value_t = 30)]
    pub request_timeout_seconds: u64,
    #[arg(long, default_value_t = 10)]
    pub sample_size: usize,
    #[arg(long, default_value_t = 12)]
    pub confirmations: u64,
    #[arg(long, value_enum, default_value_t = FinalityPolicy::Confirmations)]
    pub finality_policy: FinalityPolicy,
    #[arg(long)]
    pub allow_partial_history: bool,
    #[arg(long)]
    pub authorization_file: Option<PathBuf>,
    #[arg(long, env = "GITHUB_SHA", default_value = "local")]
    pub verified_commit: String,
}

pub async fn run(args: Args) -> Result<()> {
    validate_args(&args)?;
    let _lock = OutputLock::acquire(&args.output)?;
    let provider = ProviderBuilder::new()
        .connect(&args.rpc_url)
        .await
        .map_err(|error| rpc_error(&args.rpc_url, error.to_string()))?;
    let source_chain_id = provider
        .get_chain_id()
        .await
        .map_err(|error| rpc_error(&args.rpc_url, error.to_string()))?;
    validate_contract_history(&provider, &args).await?;
    enforce_finality(&provider, &args).await?;
    let boundary_hash = block_hash(&provider, args.snapshot_block, &args.rpc_url).await?;
    let config = CheckpointConfig {
        migration_id: args.migration_id,
        source_chain_id,
        source_contract: args.contract,
        standard: args.standard,
        from_block: args.from_block,
        snapshot_block: args.snapshot_block,
        snapshot_block_hash: boundary_hash,
        chunk_size: args.chunk_size,
        finality_policy: args.finality_policy,
        confirmations: args.confirmations,
    };
    let checkpoint_path = args
        .checkpoint
        .clone()
        .unwrap_or_else(|| args.output.join(".snapshot-checkpoint.json"));
    let mut checkpoint = load_checkpoint(&checkpoint_path, &config)?;
    fetch_missing_chunks(&provider, &args, &checkpoint_path, &mut checkpoint).await?;

    let final_hash = block_hash(&provider, args.snapshot_block, &args.rpc_url).await?;
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
    let authorization_file = args
        .authorization_file
        .as_deref()
        .map(artifacts::read_json)
        .transpose()?;
    let authorizations = authorization::resolve(
        &provider,
        &authorization::AuthorizationDomain {
            migration_id: args.migration_id,
            source_chain_id,
            source_contract: args.contract,
            snapshot_block: args.snapshot_block,
            source_block_hash: boundary_hash,
            destination_chain_id: args.destination_chain_id,
        },
        holdings.iter().map(|holding| holding.owner),
        authorization_file,
        &args.rpc_url,
    )
    .await?;
    let entries = holdings
        .into_iter()
        .enumerate()
        .map(|(index, holding)| {
            let owner = address_hex(holding.owner);
            let authorization = &authorizations[&holding.owner];
            Ok(ManifestEntry {
                standard: args.standard.code(),
                token_id: holding.token_id.to_string(),
                amount: holding.amount.to_string(),
                source_owner: owner,
                claim_authority: address_hex(authorization.claim_authority),
                destination_recipient: address_hex(authorization.destination_recipient),
                leaf_index: index as u64,
                leaf_hash: String::new(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = Manifest {
        format: "evm-migration-manifest-v2".into(),
        campaign: merkle::campaign(
            args.migration_id,
            source_chain_id,
            args.contract,
            args.snapshot_block,
            boundary_hash,
            args.destination_chain_id,
            args.standard.code(),
            match args.finality_policy {
                FinalityPolicy::Confirmations => format!("confirmations:{}", args.confirmations),
                policy => policy.to_string(),
            },
        ),
        entries,
    };

    let reconciliation = reconcile(&provider, &args, boundary_hash, &manifest.entries).await?;
    let post_reconciliation_hash =
        block_hash(&provider, args.snapshot_block, &args.rpc_url).await?;
    if post_reconciliation_hash != boundary_hash {
        return Err(SnapshotError::BoundaryChanged {
            expected: b256_hex(boundary_hash),
            actual: b256_hex(post_reconciliation_hash),
        });
    }
    if reconciliation.status != "sample-consistent" {
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
    if args.sample_size == 0 {
        return Err(SnapshotError::Config(
            "sample size must be greater than zero".into(),
        ));
    }
    if args.request_timeout_seconds == 0 {
        return Err(SnapshotError::Config(
            "request timeout must be greater than zero".into(),
        ));
    }
    if args.from_block > args.snapshot_block {
        return Err(SnapshotError::Config(
            "from block is after snapshot block".into(),
        ));
    }
    Ok(())
}

async fn block_hash<P: Provider>(provider: &P, block_number: u64, rpc_url: &str) -> Result<B256> {
    provider
        .get_block_by_number(block_number.into())
        .await
        .map_err(|error| rpc_error(rpc_url, error))?
        .map(|block| block.header.hash)
        .ok_or(SnapshotError::MissingBlock(block_number))
}

async fn enforce_finality<P: Provider>(provider: &P, args: &Args) -> Result<()> {
    let boundary = match args.finality_policy {
        FinalityPolicy::Confirmations => {
            let latest = provider
                .get_block_number()
                .await
                .map_err(|error| rpc_error(&args.rpc_url, error))?;
            if latest < args.snapshot_block.saturating_add(args.confirmations) {
                return Err(SnapshotError::Config(format!(
                    "snapshot block {} has {} confirmations; {} required",
                    args.snapshot_block,
                    latest.saturating_sub(args.snapshot_block),
                    args.confirmations
                )));
            }
            return Ok(());
        }
        FinalityPolicy::SafeBlockTag => BlockNumberOrTag::Safe,
        FinalityPolicy::FinalizedBlockTag | FinalityPolicy::L2Finalized => {
            BlockNumberOrTag::Finalized
        }
        FinalityPolicy::ManualReviewed => {
            warn!("manual-reviewed finality policy selected; operator attestation required");
            return Ok(());
        }
    };
    let finalized = provider
        .get_block_by_number(boundary)
        .await
        .map_err(|error| rpc_error(&args.rpc_url, error))?
        .ok_or_else(|| SnapshotError::Config(format!("RPC did not return {boundary} block")))?;
    if args.snapshot_block > finalized.header.number {
        return Err(SnapshotError::Config(format!(
            "snapshot block {} is newer than {} block {}",
            args.snapshot_block, args.finality_policy, finalized.header.number
        )));
    }
    Ok(())
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
            let transfers = fetch_range(&provider, args, start, end).await?;
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

async fn fetch_range<P: Provider>(
    provider: &P,
    args: &Args,
    start: u64,
    end: u64,
) -> Result<Vec<Transfer>> {
    let mut pending = vec![(start, end)];
    let mut transfers = Vec::new();
    while let Some((range_start, range_end)) = pending.pop() {
        match fetch_logs(provider, args, range_start, range_end).await {
            Ok(logs) => transfers.extend(decode_logs(args.standard, logs)?),
            Err(FetchError::RangeLimit) if range_start < range_end => {
                let middle = range_start + (range_end - range_start) / 2;
                pending.push((middle + 1, range_end));
                pending.push((range_start, middle));
                warn!(
                    range_start,
                    range_end, "provider range rejected; split adaptively"
                );
            }
            Err(FetchError::RangeLimit) => {
                return Err(SnapshotError::Rpc(format!(
                    "provider rejected single-block log range {range_start}"
                )));
            }
            Err(FetchError::Permanent(message)) => return Err(SnapshotError::Rpc(message)),
        }
    }
    transfers.sort_by(|left, right| left.position.cmp(&right.position));
    let mut positions = BTreeSet::new();
    if let Some(duplicate) = transfers
        .iter()
        .find(|transfer| !positions.insert(transfer.position.clone()))
    {
        return Err(SnapshotError::MalformedLog(format!(
            "duplicate event position {:?}",
            duplicate.position
        )));
    }
    Ok(transfers)
}

enum FetchError {
    RangeLimit,
    Permanent(String),
}

async fn fetch_logs<P: Provider>(
    provider: &P,
    args: &Args,
    start: u64,
    end: u64,
) -> std::result::Result<Vec<Log>, FetchError> {
    let signatures = match args.standard {
        TokenStandard::Erc721 => vec![erc721::Transfer::SIGNATURE_HASH],
        TokenStandard::Erc1155 => vec![
            erc1155::TransferSingle::SIGNATURE_HASH,
            erc1155::TransferBatch::SIGNATURE_HASH,
        ],
    };
    let filter = Filter::new()
        .address(args.contract)
        .event_signature(signatures)
        .from_block(start)
        .to_block(end);
    for attempt in 0..4u32 {
        let response = tokio::time::timeout(
            Duration::from_secs(args.request_timeout_seconds),
            provider.get_logs(&filter),
        )
        .await;
        let message = match response {
            Ok(Ok(logs)) => return Ok(logs),
            Ok(Err(error)) => rpc_message(&args.rpc_url, &error.to_string()),
            Err(_) => format!("request timed out after {}s", args.request_timeout_seconds),
        };
        if is_range_limit(&message) {
            return Err(FetchError::RangeLimit);
        }
        if attempt == 3 || !is_transient(&message) {
            return Err(FetchError::Permanent(message));
        }
        let delay = 250u64 * (1u64 << attempt) + (start % 97);
        warn!(
            start,
            end, attempt, delay, "transient RPC failure; retrying"
        );
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    unreachable!()
}

fn is_range_limit(message: &str) -> bool {
    let value = message.to_ascii_lowercase();
    [
        "too many results",
        "block range",
        "response size",
        "query returned more than",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn is_transient(message: &str) -> bool {
    let value = message.to_ascii_lowercase();
    [
        "429",
        "timeout",
        "timed out",
        "temporarily unavailable",
        "connection reset",
        "502",
        "503",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

async fn validate_contract_history<P: Provider>(provider: &P, args: &Args) -> Result<()> {
    let code = provider
        .get_code_at(args.contract)
        .block_id(BlockId::from(args.snapshot_block))
        .await
        .map_err(|error| rpc_error(&args.rpc_url, error.to_string()))?;
    if code.is_empty() {
        return Err(SnapshotError::Config(
            "source contract has no code at the snapshot block".into(),
        ));
    }
    if args.from_block > 0 && !args.allow_partial_history {
        let earlier = provider
            .get_code_at(args.contract)
            .block_id(BlockId::from(args.from_block - 1))
            .await
            .map_err(|error| rpc_error(&args.rpc_url, error.to_string()))?;
        if !earlier.is_empty() {
            return Err(SnapshotError::Config(
                "from-block is later than contract creation; use the creation block or explicitly pass --allow-partial-history"
                    .into(),
            ));
        }
    }
    Ok(())
}

struct OutputLock(PathBuf);

impl OutputLock {
    fn acquire(output: &Path) -> Result<Self> {
        fs::create_dir_all(output).map_err(|source| SnapshotError::Io {
            path: output.to_path_buf(),
            source,
        })?;
        let path = output.join(".snapshot.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|source| SnapshotError::Io {
                path: path.clone(),
                source,
            })?;
        Ok(Self(path))
    }
}

impl Drop for OutputLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
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

async fn reconcile<P: Provider + Clone>(
    provider: &P,
    args: &Args,
    boundary_hash: B256,
    entries: &[ManifestEntry],
) -> Result<Reconciliation> {
    let indices = sample_indices(entries, args.sample_size, boundary_hash);
    let sampled_entries = indices.len();
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
                    .map_err(|error| rpc_error(&args.rpc_url, error))?;
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
                    .map_err(|error| rpc_error(&args.rpc_url, error))?
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
        "sample-consistent"
    } else {
        "inconsistent"
    };
    Ok(Reconciliation {
        snapshot_block: args.snapshot_block,
        snapshot_block_hash: b256_hex(boundary_hash),
        samples,
        status: status.into(),
        sampled_entries,
        manifest_entries: entries.len(),
        sample_coverage: sampled_entries as f64 / entries.len().max(1) as f64,
        sample_seed: b256_hex(boundary_hash),
        providers_compared: 1,
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
                claim_authority: address_hex(Address::ZERO),
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

    #[test]
    fn classifies_provider_failures_and_redacts_urls() {
        assert!(is_range_limit("query returned more than 10000 results"));
        assert!(is_transient("HTTP 429 temporarily unavailable"));
        assert_eq!(
            rpc_message(
                "https://rpc.example/key",
                "timeout at https://rpc.example/key"
            ),
            "timeout at <rpc-url>"
        );
    }
}
