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
  --confirmations 64 \
  --output artifacts/erc1155
```

Interrupted runs resume from `artifacts/erc1155/.snapshot-checkpoint.json`. Do not edit it. A different configuration or block hash is rejected.

Review `manifest.json`, `root.txt`, `proofs.json`, `reconciliation.json`, and `summary.md`. Repeat against a second archive RPC and compare artifacts byte-for-byte. Increase the sample size for production.

## 3. Sepolia to Base Sepolia rehearsal

Import an encrypted keystore; never place a private key in shell history or `.env`:

```bash
cast wallet import migration-deployer --interactive
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
export SOURCE_CHAIN_ID=11155111 SNAPSHOT_BLOCK=...
export CLAIM_START=... CLAIM_DEADLINE=...
forge script contracts/script/DeployDestination.s.sol:DeployDestination \
  --root contracts --rpc-url base_sepolia --account migration-deployer \
  --sender "$DEPLOYER_ADDRESS" --broadcast --verify
```

Register the reviewed root from the admin account. If the admin is a Safe/timelock, submit the equivalent `setRoot(root, 1)` calldata through it instead of broadcasting this script:

```bash
export MIGRATION_CLAIM=0x... MERKLE_ROOT=0x... ROOT_VERSION=1
forge script contracts/script/RegisterRoot.s.sol:RegisterRoot \
  --root contracts --rpc-url base_sepolia --account migration-deployer \
  --sender "$ADMIN_ADDRESS" --broadcast
```

Etherscan uses its V2 API for both chain IDs. Base Sepolia verification currently requires a paid Etherscan tier; a failed verification must be resolved before the deployment is called complete.

## 4. Canary and launch

- Verify immutable campaign fields, token minters, owner/Safe, root, version, and claim timestamps from two independent clients.
- Serve the exact reviewed artifacts in `apps/web/public/campaign` and configure `VITE_CLAIM_ADDRESS`, chain ID `84532`, and the Base Sepolia RPC.
- Exercise one direct, one batch, one delegated EOA, and one ERC-1271 claim.
- Compare `claimedCount`, bitmap status, emitted `Claimed` events, and token ownership/balances to the manifest.
- Publish contract addresses, artifact hashes, source block/hash, CI commit, and correction policy.

## 5. Operate and close

Monitor `RootUpdated`, `Paused`, `Unpaused`, and `Claimed`. Reconcile manifest entries with claimed bits/events. Any root correction follows the public procedure in the threat model. At deadline, publish final counts and a status artifact; the immutable window closes without an admin transaction.

## Release gate

Do not tag a stable release or call the repository complete until Sepolia and Base Sepolia contracts are verified, the live UI points at them, the canary matrix passes, CI is green, and the deployment table in the README contains real addresses.
