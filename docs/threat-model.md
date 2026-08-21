# Threat model

## Assets and actors

Assets are destination mint authority, unclaimed allocations, root integrity, delegated signatures, deterministic artifacts, and the availability of a bounded claim window. Actors include source holders, claim recipients, relayers, the manifest operator, RPC provider, root/token administrator, and malicious third parties.

## Security properties

- A valid leaf commits to both chain domains, the source block hash, token data, historical owner, destination claim authority, fixed recipient, and unique index.
- A leaf index is usable once for the lifetime of the campaign.
- Direct and batch claims can only be initiated by their committed destination authority.
- Delegated claims cannot be redirected, replayed across nonces, used after their deadline, or replayed on another chain/contract domain.
- Pausing blocks every claim path; the claim window cannot be extended after deployment.
- Only the permanently configured claim contract can mint destination tokens.

## Threats and mitigations

| Threat | Mitigation | Residual trust/risk |
| --- | --- | --- |
| Forged campaign fields | Contract reconstructs leaves from immutables | Root can still omit or misassign state |
| Duplicate or replayed claim | Global bitmap; delegated nonce, deadline, root, and version | Post-launch corrections require redeployment |
| Signature used for another recipient | Recipient is in both leaf and EIP-712 message | User still must inspect the typed-data domain |
| Reorged snapshot | Explicit finality policy, leaf-bound block hash, hash checks before and after all historical reads | RPC can lie consistently |
| Missing/malformed event history | Explicit decoding errors, deterministic state math, sampled historical reads | Sampling does not prove every holding |
| Unbounded RPC fan-out | Configurable bounded Tokio concurrency and atomic checkpoints | Provider limits may still require smaller chunks |
| Reentrancy through mint or ERC-1271 | `nonReentrant`; signature validates before mutation and claim bit is set before mint | ERC-1155 receiver rejection can make its own claim revert |
| Batch gas griefing | Caller pays gas; atomic transaction; no operator loop | Very large owners may need single claims |
| Compromised admin | Safe/timelock policy, two-step token ownership, pre-launch-only roots, pause | Admin can corrupt a pre-launch root or pause claims |
| Frontend artifact tampering | Runtime schema/digest/proof checks plus full onchain campaign comparison; fail closed | A malicious host can censor availability |

Timestamp drift around the claim boundary is accepted because it can shift availability by seconds, not redirect assets. Receiver callback failures only affect the caller's transaction.

## Root correction procedure

Before claims open, cancel the launch, publish the old/new manifest diff and bundle digests, increment the root version, and obtain the same administrative approvals. Once `claimStart` is reached, the contract rejects root changes. A later incident requires a new campaign deployment; there is no automatic rollback. Never silently replace a root.

## Out of scope

- Trustless source-chain consensus or light-client verification
- Ongoing bidirectional bridging or source-token escrow/burn
- Recovery of keys, signatures obtained through phishing, or compromised admin quorum
- Non-standard tokens whose balances cannot be reconstructed from standard transfer events
- Rebasing, fee-on-transfer, soulbound, or callback-dependent token semantics
- Automatic relaying, gas sponsorship, sanctions policy, and identity checks
- Mainnet deployment and economic audit certification
