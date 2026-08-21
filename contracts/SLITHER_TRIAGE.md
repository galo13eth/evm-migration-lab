# Slither triage

Slither runs against production contracts only. Findings are fixed unless explicitly accepted here.

| Detector | Location | Disposition |
| --- | --- | --- |
| `calls-loop` | Typed claim `_mint` calls reachable from `claimBatch` | Accepted inline. Batch size is caller-funded and bounded by block gas. The function is atomic and non-reentrant. |
| `costly-loop` | `MigrationClaimBase._completeClaim` cumulative count | Accepted inline. One storage update per claimed leaf is the intended reconciliation state. |
| `timestamp` | `MigrationClaimBase` constructor, window modifier, `setRoot`, and delegated deadline | Accepted inline. Minute-scale timestamp drift cannot redirect assets; it only shifts explicit launch/deadline boundaries. |

ERC-1155 receiver callbacks and ERC-1271 validation are intentional. Every claim entry is marked before minting, delegated claims are nonce-protected, and all claim entry points use `nonReentrant`. ERC-1271 validation occurs before nonce and bitmap updates; `nonReentrant` protects that interaction.

The Slither configuration excludes only compiler/style findings. Security-relevant exceptions are suppressed at their exact source locations so new findings elsewhere remain visible.
