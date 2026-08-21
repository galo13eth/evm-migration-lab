# Migration runbook

This runbook is deliberately operator-driven. Every root should have a human-readable change record and independent review before it reaches the destination chain.

## 1. Prepare

- Choose one source contract and token standard per campaign.
- Announce the late-transfer policy and freeze block selection rule.
- Confirm archive access for historical `eth_getLogs`, `ownerOf`, and `balanceOf`.
- Create the destination Safe/timelock and document signers, threshold, pause authority, claim window, and root-correction policy.
- Run `make demo` from a clean checkout.

## 2. Select and reconstruct

Wait for the chosen source block to exceed the required confirmation/finality threshold. Record its number and hash independently. Run the CLI with a dedicated output directory:

```bash
cargo run --locked -p evm-snapshot -- \
  --rpc-url "$SEPOLIA_RPC_URL" \
  --contract "$SOURCE_CONTRACT" \
  --standard erc1155 \
  --snapshot-block "$SNAPSHOT_BLOCK" \
  --migration-id "$MIGRATION_ID" \
  --destination-chain-id 84532 \
  --confirmations 64 \
  --sample-size 100 \
  --output artifacts/erc1155
```

Interrupted runs resume from `artifacts/erc1155/.snapshot-checkpoint.json`. Do not edit it. A different configuration or block hash is rejected.

Resolve `artifacts/erc1155/current.json`, require the bundle's `READY` marker, then review `manifest.json`, `root.txt`, `proofs.json`, `reconciliation.json`, `summary.md`, and `artifact-digests.json` together. `sample-consistent` describes only the sampled historical reads. Repeat against a second archive RPC and require `./scripts/compare-snapshot-bundles.sh <provider-a-bundle> <provider-b-bundle>` to pass before approval.

Every source contract wallet must provide `--authorization-file`; snapshot generation rejects contract owners without one. The file must use `evm-migration-authorizations-v1`; each override signs `MigrationAuthorization` for the exact migration ID, source contract/block/hash, destination chain, authority, and recipient. The CLI validates EOAs locally and ERC-1271 wallets against source-chain code at the snapshot block. Never use an unsigned recipient map.

## 3. Sepolia to Base Sepolia rehearsal

Import an encrypted keystore; never place a private key in shell history or `.env`:

```bash
cast wallet import migration-deployer --interactive
cast wallet import migration-admin --interactive # only for a separate admin EOA
export DEPLOYER_ADDRESS=0x...
export ADMIN_ADDRESS=0x... # Safe/timelock
export SEPOLIA_RPC_URL=https://...
export BASE_SEPOLIA_RPC_URL=https://...
export ETHERSCAN_API_KEY=...
```

Deploy and seed the source fixture on Sepolia:

```bash
export DEMO_OWNER_A=0x... DEMO_OWNER_B=0x... DEMO_OWNER_C=0x...
forge script contracts/script/DeploySource.s.sol:DeploySource \
  --root contracts --rpc-url sepolia --account migration-deployer \
  --sender "$DEPLOYER_ADDRESS" --broadcast --verify
```

After snapshot generation, deploy the destination campaign on Base Sepolia. `SOURCE_CONTRACT` must be the one contract represented by this manifest:

```bash
export SOURCE_CHAIN_ID=11155111 SNAPSHOT_BLOCK=... SOURCE_BLOCK_HASH=0x...
export CAMPAIGN_STANDARD=2 # deploy ERC-1155 only; use 1 for ERC-721
# Optional overrides. Defaults are a 24-hour lead and 30-day window.
export CLAIM_START=... CLAIM_DEADLINE=...
forge script contracts/script/DeployDestination.s.sol:DeployDestination \
  --root contracts --rpc-url base_sepolia --account migration-deployer \
  --sender "$DEPLOYER_ADDRESS" --broadcast --verify
```

When `ADMIN_ADDRESS != DEPLOYER_ADDRESS`, accept the token's `Ownable2Step` handoff from the administrator. For an EOA use its own encrypted account; for a Safe/timelock submit `acceptOwnership()` through that system:

```bash
export DESTINATION_TOKEN=0x...
forge script contracts/script/AcceptTokenOwnership.s.sol:AcceptTokenOwnership \
  --root contracts --rpc-url base_sepolia --account migration-admin \
  --sender "$ADMIN_ADDRESS" --broadcast
cast call "$DESTINATION_TOKEN" 'owner()(address)' --rpc-url "$BASE_SEPOLIA_RPC_URL"
cast call "$DESTINATION_TOKEN" 'pendingOwner()(address)' --rpc-url "$BASE_SEPOLIA_RPC_URL"
```

Require `owner() == ADMIN_ADDRESS`, `pendingOwner() == address(0)`, `minter() == MIGRATION_CLAIM`, and `minterLocked() == true` before proceeding.

Register the reviewed root and bundle digest from the admin account. If the admin is a Safe/timelock, submit the equivalent `setRoot(root, artifactDigest, 1)` calldata through it instead of broadcasting this script:

```bash
export MIGRATION_CLAIM=0x... MERKLE_ROOT=0x... ARTIFACT_DIGEST=0x... ROOT_VERSION=1
forge script contracts/script/RegisterRoot.s.sol:RegisterRoot \
  --root contracts --rpc-url base_sepolia --account migration-admin \
  --sender "$ADMIN_ADDRESS" --broadcast
```

Etherscan uses its V2 API for both chain IDs. Base Sepolia verification currently requires a paid Etherscan tier; a failed verification must be resolved before the deployment is called complete.

## 4. Canary and launch

- Verify every constructor field, bytecode, token minter/lock, owner/pending owner, root, artifact digest, version, pause state, and claim window from two independent clients before root registration.
- Serve the exact reviewed artifacts in `apps/web/public/campaign` and configure `VITE_CLAIM_ADDRESS`, chain ID `84532`, and the Base Sepolia RPC.
- Exercise ERC-721 and ERC-1155 direct/batch claims, delegated EOA, destination-deployed ERC-1271, and a source-only ERC-1271 authorization.
- Compare `claimedCount`, bitmap status, emitted `Claimed` events, and token ownership/balances to the manifest.
- Publish contract addresses, artifact hashes, source block/hash, CI commit, and correction policy.
- After canary verification, call `freezeMetadata()` or document the retained metadata authority and timelock policy.

## 5. Operate and close

Monitor `RootUpdated`, `Paused`, `Unpaused`, and `Claimed`. Reconcile manifest entries with claimed bits/events. Any root correction follows the public procedure in the threat model. At deadline, publish final counts and a status artifact; the immutable window closes without an admin transaction.

## Release gate

Do not tag a stable release or call the repository complete until Sepolia and Base Sepolia contracts are verified, the live UI points at them, the canary matrix passes, CI is green, and the deployment table in the README contains real addresses.
