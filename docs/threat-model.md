# Threat model

## Assets and actors

Assets are destination mint authority, unclaimed allocations, root integrity, delegated signatures, deterministic artifacts, and the availability of a bounded claim window. Actors include source holders, claim recipients, relayers, the manifest operator, RPC provider, root/token administrator, and malicious third parties.

## Security properties

- A valid leaf commits to the full source campaign, token data, source owner, fixed destination recipient, and unique index.
- A leaf index is usable once per root version.
- Direct and batch claims can only be initiated by their source owner.
- Delegated claims cannot be redirected, replayed across nonces, used after their deadline, or replayed on another chain/contract domain.
- Pausing blocks every claim path; the claim window cannot be extended after deployment.
- Only the permanently configured claim contract can mint destination tokens.

## Threats and mitigations

| Threat | Mitigation | Residual trust/risk |
| --- | --- | --- |
| Forged campaign fields | Contract reconstructs leaves from immutables | Root can still omit or misassign state |
| Duplicate or replayed claim | Versioned bitmap; delegated nonce and deadline | A new root version is a new claim namespace |
| Signature used for another recipient | Recipient is in both leaf and EIP-712 message | User still must inspect the typed-data domain |
| Reorged snapshot | Confirmation threshold, stored block hash, hash recheck on completion/resume | RPC can lie consistently |
| Missing/malformed event history | Explicit decoding errors, deterministic state math, sampled historical reads | Sampling does not prove every holding |
| Unbounded RPC fan-out | Configurable bounded Tokio concurrency and atomic checkpoints | Provider limits may still require smaller chunks |
| Reentrancy through safe mint or ERC-1271 | `nonReentrant`; claim bit set before mint | Hostile receivers can make their own claim revert |
| Batch gas griefing | Caller pays gas; atomic transaction; no operator loop | Very large owners may need single claims |
| Compromised admin | Safe/timelock policy, two-step ownership, root/version events, pause | Admin can replace roots or pause claims |
| Frontend artifact tampering | Root remains authoritative on-chain; publish hashes and commit | A malicious UI can censor or confuse users |

Timestamp drift around the claim boundary is accepted because it can shift availability by seconds, not redirect assets. Receiver callback failures only affect the caller's transaction.

## Root correction procedure

Before claims open, cancel the launch, publish the manifest diff, increment the root version, and obtain the same administrative approvals. After any successful claim, a replacement root requires a public incident decision about already minted supply; there is no automatic rollback. Never silently replace a root.

## Out of scope

- Trustless source-chain consensus or light-client verification
- Ongoing bidirectional bridging or source-token escrow/burn
- Recovery of keys, signatures obtained through phishing, or compromised admin quorum
- Non-standard tokens whose balances cannot be reconstructed from standard transfer events
- Rebasing, fee-on-transfer, soulbound, or callback-dependent token semantics
- Automatic relaying, gas sponsorship, sanctions policy, and identity checks
- Mainnet deployment and economic audit certification
