# Slither triage

Slither runs against production contracts only. Findings are fixed unless explicitly accepted here.

| Detector | Location | Disposition |
| --- | --- | --- |
| `calls-loop` | `MigrationClaim.claimBatch` | Accepted. Batch size is caller-funded and bounded by block gas. The function is atomic and non-reentrant. |
| `timestamp` | Claim window and delegated deadline checks | Accepted. Minute-scale timestamp drift cannot redirect assets; it only shifts an explicitly administrative time boundary. |
| `missing-inheritance` | Destination tokens and claim-local mint interfaces | Accepted. The narrow interfaces deliberately keep the claim contract decoupled from concrete token implementations; ABI conformance is exercised by tests and the e2e pipeline. |

Receiver callbacks from ERC-721/1155 safe minting and ERC-1271 validation are intentional. Every claim entry is marked before minting, delegated claims are nonce-protected, and all claim entry points use `nonReentrant`.
