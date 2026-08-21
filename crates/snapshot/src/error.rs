use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SnapshotError>;

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("RPC request failed: {0}")]
    Rpc(String),
    #[error("snapshot block {0} was not returned by the RPC")]
    MissingBlock(u64),
    #[error("snapshot boundary changed: expected {expected}, received {actual}")]
    BoundaryChanged { expected: String, actual: String },
    #[error("malformed token log: {0}")]
    MalformedLog(String),
    #[error("ownership reconstruction failed: {0}")]
    Reconstruction(String),
    #[error("sampled on-chain state disagrees with the reconstruction")]
    Reconciliation,
    #[error("migration authorization failed: {0}")]
    Authorization(String),
    #[error("Merkle tree failed: {0}")]
    Merkle(String),
    #[error("checkpoint parameters do not match this run: {0}")]
    CheckpointMismatch(String),
    #[error("failed to read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub(crate) fn rpc_message(rpc_url: &str, message: &str) -> String {
    message.replace(rpc_url, "<rpc-url>")
}

pub(crate) fn rpc_error(rpc_url: &str, error: impl ToString) -> SnapshotError {
    SnapshotError::Rpc(rpc_message(rpc_url, &error.to_string()))
}
