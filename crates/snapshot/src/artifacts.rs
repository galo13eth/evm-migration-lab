use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::model::{Manifest, ProofBundle, Reconciliation, StatusArtifact};
use crate::{Result, SnapshotError};

pub fn write_reconciliation(output: &Path, reconciliation: &Reconciliation) -> Result<()> {
    fs::create_dir_all(output).map_err(|source| SnapshotError::Io {
        path: output.to_path_buf(),
        source,
    })?;
    write_json(&output.join("reconciliation.json"), reconciliation)?;
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
    write_atomic(&output.join("reconciliation.md"), report.as_bytes())
}

pub fn write_success(
    output: &Path,
    manifest: &Manifest,
    proofs: &ProofBundle,
    reconciliation: &Reconciliation,
    verified_commit: &str,
) -> Result<()> {
    fs::create_dir_all(output).map_err(|source| SnapshotError::Io {
        path: output.to_path_buf(),
        source,
    })?;
    write_json(&output.join("manifest.json"), manifest)?;
    write_json(&output.join("proofs.json"), proofs)?;
    write_atomic(
        &output.join("root.txt"),
        format!("{}\n", proofs.root).as_bytes(),
    )?;
    write_reconciliation(output, reconciliation)?;
    write_json(
        &output.join("status.json"),
        &StatusArtifact {
            snapshot_block: manifest.campaign.snapshot_block,
            manifest_entries: manifest.entries.len(),
            merkle_root: proofs.root.clone(),
            claims_completed: 0,
            reconciliation_status: reconciliation.status.clone(),
            last_verified_commit: verified_commit.to_owned(),
        },
    )?;
    write_atomic(
        &output.join("summary.md"),
        format!(
            "# Snapshot summary\n\n- Standard: `{}`\n- Entries: `{}`\n- Merkle root: `{}`\n- Reconciliation: **{}**\n",
            manifest.campaign.standard,
            manifest.entries.len(),
            proofs.root,
            reconciliation.status
        )
        .as_bytes(),
    )
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
