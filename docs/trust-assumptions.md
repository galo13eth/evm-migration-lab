# Trust assumptions

**This system is not a trustless bridge.** Users trust the snapshot process and root administrator to commit the intended source state.

## Required trust

1. The selected source RPC returns canonical logs and historical contract state at the declared block.
2. The operator chooses a sufficiently finalized block and does not treat later source transfers as migratable.
3. The published manifest, proofs, reconciliation report, and registered root are reviewed as one artifact set.
4. The root administrator follows the announced correction policy and claim window.
5. Destination token metadata and administrative keys are operated as disclosed.

## On-chain guarantees

Once a root is registered, the contract verifies all immutable campaign fields, owner, fixed recipient, amount, and leaf index. It prevents a leaf index from claiming twice within that root version, enforces the claim window and pause state, and protects delegated claims with EIP-712 domain separation, nonces, deadlines, and EOA/ERC-1271 validation.

The contract cannot prove that the root represents the canonical source chain. It cannot detect omitted owners, operator-selected recipients, or an improperly chosen snapshot block.

## Administration policy

Production ownership should be a Safe, preferably behind a timelock that gives users time to inspect root changes. A root update is for a pre-launch correction or a publicly documented emergency only. Because every new version has a fresh claim bitmap, an administrator could make assets claimable again under a replacement root. Operators must publish both versions, the reason, diff, approvals, and treatment of already minted supply.

Emergency pause authority can stop new claims but cannot reverse completed mints. Token ownership can update metadata; the minter address is one-time and permanently locked.

## Late transfers

Ownership at snapshot block `N` is final for the campaign. Transfers on the source chain after `N` do not change eligibility. Operators should communicate the freeze before `N`; users must treat post-snapshot source assets and destination claims as separate state.
