# Changelog

All notable changes are documented here. This project follows [Semantic Versioning](https://semver.org/) after its first stable tag.

## Unreleased

## 0.1.0 - 2026-08-21

- Split destination claims into ERC-721 and ERC-1155 campaigns with one permanently locked minter each.
- Added source block-hash and destination-chain commitments, source-wallet authorization, and root/version-bound delegated signatures.
- Froze roots at launch, made claim tracking cumulative, and added artifact-bundle digests.
- Added resilient Rust RPC backfill, finality policies, source authorization validation, atomic output bundles, and sample-qualified reconciliation.
- Made the web application validate artifacts and complete onchain campaign state before enabling actions.
- Expanded Solidity, Rust, frontend, and two-chain evidence and hardened CI/dependency pinning.
- Published the independently reproduced Sepolia → Base Sepolia canary, verified contracts, static artifact bundles, live UI, and transaction-level reconciliation report.
