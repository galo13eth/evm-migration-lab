#![forbid(unsafe_code)]

use clap::Parser;
use evm_snapshot::{Result, rpc};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();
    rpc::run(rpc::Args::parse()).await
}
