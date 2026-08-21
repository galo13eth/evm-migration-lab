# Trust assumptions

**This system is not a trustless bridge.** Users trust the snapshot process and root administrator to commit the intended source state.

## Required trust

1. The selected source RPC returns canonical logs and historical contract state at the declared block.
2. The operator chooses a sufficiently finalized block and does not treat later source transfers as migratable.
3. The published manifest, proofs, reconciliation report, and registered root are reviewed as one artifact set.
4. The root administrator follows the announced correction policy and claim window.
5. Destination token metadata and administrative keys are operated as disclosed.

## On-chain guarantees

Once a root is registered, the contract verifies both chain domains, source block hash, historical owner, destination authority, fixed recipient, amount, and leaf index. It prevents a leaf index from claiming twice for the campaign lifetime, enforces the immutable claim window and pause state, and protects delegated claims with root/version-bound EIP-712 signatures, nonces, deadlines, and EOA/ERC-1271 validation.

The contract cannot prove that the root represents the canonical source chain. It cannot detect omitted owners, operator-selected recipients, or an improperly chosen snapshot block.

## Administration policy

Production ownership should be a Safe, preferably behind a timelock. A root update is possible only before launch and before any claim; operators must publish both versions, bundle digests, the reason, diff, and approvals. Post-launch correction requires redeployment.

Emergency pause authority can stop new claims but cannot reverse completed mints. Token ownership can update metadata until `freezeMetadata()` is called; the minter address is one-time and permanently locked. The runbook records whether metadata is frozen, timelocked, or intentionally retained.

An EOA controls the same address on both EVM chains. A contract wallet may not. Source-only ERC-1271 owners therefore authorize a destination `claimAuthority` in a source-chain EIP-712 message validated by the CLI at the snapshot block. Delegated claim signatures from a contract authority work only when that ERC-1271 account is deployed and valid on the destination chain.

## Late transfers

Ownership at snapshot block `N` is final for the campaign. Transfers on the source chain after `N` do not change eligibility. Operators should communicate the freeze before `N`; users must treat post-snapshot source assets and destination claims as separate state.
