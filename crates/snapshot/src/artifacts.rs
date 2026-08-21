use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use alloy::primitives::keccak256;
use serde::Serialize;

use crate::model::{
    ArtifactDigests, CurrentBundle, Manifest, ProofBundle, Reconciliation, StatusArtifact, b256_hex,
};
use crate::{Result, SnapshotError};

pub fn write_success(
    output: &Path,
    manifest: &Manifest,
    proofs: &ProofBundle,
    reconciliation: &Reconciliation,
    verified_commit: &str,
) -> Result<()> {
    let reconciliation_report = reconciliation_markdown(reconciliation);
    let summary = format!(
        "# Snapshot summary\n\n- Standard: `{}`\n- Entries: `{}`\n- Merkle root: `{}`\n- Reconciliation: **{}**\n",
        manifest.campaign.standard,
        manifest.entries.len(),
        proofs.root,
        reconciliation.status
    );
    let committed: BTreeMap<String, Vec<u8>> = BTreeMap::from([
        ("manifest.json".into(), json_bytes(manifest)?),
        ("proofs.json".into(), json_bytes(proofs)?),
        ("reconciliation.json".into(), json_bytes(reconciliation)?),
        (
            "reconciliation.md".into(),
            reconciliation_report.into_bytes(),
        ),
        ("root.txt".into(), format!("{}\n", proofs.root).into_bytes()),
        ("summary.md".into(), summary.into_bytes()),
    ]);
    let files = committed
        .iter()
        .map(|(name, bytes)| (name.clone(), b256_hex(keccak256(bytes))))
        .collect::<BTreeMap<_, _>>();
    let mut bundle_preimage = Vec::new();
    for (name, digest) in &files {
        bundle_preimage.extend_from_slice(name.as_bytes());
        bundle_preimage.extend_from_slice(digest.as_bytes());
    }
    let bundle_digest = b256_hex(keccak256(bundle_preimage));
    let run_id = bundle_digest.trim_start_matches("0x");
    let runs = output.join("runs");
    let published = runs.join(run_id);
    if published.exists() {
        return write_json(
            &output.join("current.json"),
            &CurrentBundle {
                format: "evm-migration-current-bundle-v1".into(),
                merkle_root: proofs.root.clone(),
                path: format!("runs/{run_id}"),
            },
        );
    }
    let staging = runs.join(format!(".{run_id}-{}.tmp", std::process::id()));
    fs::create_dir_all(&staging).map_err(|source| SnapshotError::Io {
        path: staging.clone(),
        source,
    })?;
    for (name, bytes) in committed {
        write_atomic(&staging.join(name), &bytes)?;
    }
    write_json(
        &staging.join("artifact-digests.json"),
        &ArtifactDigests {
            format: "evm-migration-artifact-digests-v1".into(),
            files,
            bundle_digest: bundle_digest.clone(),
            cli_version: env!("CARGO_PKG_VERSION").into(),
            verified_commit: verified_commit.into(),
            source_block: manifest.campaign.snapshot_block,
            source_block_hash: manifest.campaign.snapshot_block_hash.clone(),
        },
    )?;
    write_json(
        &staging.join("status.json"),
        &StatusArtifact {
            environment: "snapshot-artifact".into(),
            chain_id: manifest.campaign.destination_chain_id,
            live: false,
            generated_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| SnapshotError::Config(error.to_string()))?
                .as_secs(),
            snapshot_block: manifest.campaign.snapshot_block,
            snapshot_block_hash: manifest.campaign.snapshot_block_hash.clone(),
            manifest_entries: manifest.entries.len(),
            merkle_root: proofs.root.clone(),
            claims_completed: 0,
            reconciliation_status: reconciliation.status.clone(),
            last_verified_commit: verified_commit.to_owned(),
        },
    )?;
    write_atomic(&staging.join("READY"), b"ready\n")?;

    fs::rename(&staging, &published).map_err(|source| SnapshotError::Io {
        path: published.clone(),
        source,
    })?;
    write_json(
        &output.join("current.json"),
        &CurrentBundle {
            format: "evm-migration-current-bundle-v1".into(),
            merkle_root: proofs.root.clone(),
            path: format!("runs/{run_id}"),
        },
    )
}

fn reconciliation_markdown(reconciliation: &Reconciliation) -> String {
    let mut report = format!(
        "# Snapshot reconciliation\n\n- Snapshot block: `{}`\n- Boundary hash: `{}`\n- Samples: `{}`\n- Status: **{}**\n\n",
        reconciliation.snapshot_block,
        reconciliation.snapshot_block_hash,
        reconciliation.samples.len(),
        reconciliation.status
    );
    for sample in &reconciliation.samples {
        report.push_str(&format!(
            "- Leaf `{}`: expected `{}`, received `{}` — {}\n",
            sample.leaf_index,
            sample.expected,
            sample.actual,
            if sample.consistent {
                "consistent"
            } else {
                "MISMATCH"
            }
        ));
    }
    report
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| SnapshotError::Json {
        path: "<memory>".into(),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|source| SnapshotError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SnapshotError::Json {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| SnapshotError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SnapshotError::Config(format!("invalid output path: {}", path.display())))?;
    let temporary = path.with_file_name(format!(".{file_name}.tmp"));
    fs::write(&temporary, bytes).map_err(|source| SnapshotError::Io {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, path).map_err(|source| SnapshotError::Io {
        path: path.to_path_buf(),
        source,
    })
}
