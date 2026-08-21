# Sepolia → Base Sepolia release canary

This report records the public `v0.1.0` canary for the snapshot-and-claim pipeline. It is testnet evidence, not a claim that the system is a trustless bridge or has received an independent audit.

## Campaign boundary and artifacts

Both typed campaigns use Sepolia block `11533768`, whose hash is `0xa6472000959ac4f2d2632e418405d3b6dbc8ef3b003683c6c5b1bf2d2fc3ff2c`. The snapshot block had 64 confirmations before final generation. Each final bundle was generated through two distinct RPC providers, fully sampled against historical contract reads, and compared byte-for-byte. The machine-readable result is [`provider-comparison.json`](../artifacts/sepolia-base-sepolia/provider-comparison.json).

| Campaign | Entries | Root version | Merkle root | Bundle digest | Reconciliation |
| --- | ---: | ---: | --- | --- | --- |
| [ERC-721](../artifacts/sepolia-base-sepolia/erc721) | 4 | 1 | `0x7f4f4d5e7dc5243fbbe7ec9ea390abd3aee3c60b1841f3017ccbaddad44217eb` | `0x786dcd69611232c1c2b1e356987d7edca3ebd1b1959a516876b5ebace09a96c2` | 4/4 sample-consistent |
| [ERC-1155](../artifacts/sepolia-base-sepolia/erc1155) | 4 | 2 | `0xc13128996bc0d6ffe808e4715c61c31baca6c9d06211f41920136d2117c3591f` | `0x969241e6971267703281ff4bc06a492b0b92bc3a801f0dba660b788e49413e86` | 4/4 sample-consistent |

The ERC-1155 root was corrected before launch with zero claims. Version 1 assigned two Owner B allocations to a destination ERC-1271 authority, which could validate delegated signatures but could not originate the intended batch transaction. Version 2 restores the EOA default for those allocations. The [old bundle](../artifacts/sepolia-base-sepolia/erc1155-v1) and [machine-readable diff](../artifacts/sepolia-base-sepolia/erc1155/root-correction-v1-to-v2.json) remain public. The 2-of-3 Safe registered the correction in [transaction `0xa577…2270`](https://sepolia.basescan.org/tx/0xa577d320022fe581c6bda8ea7095acf9b9c63b069bb3a9f89559d5f637da2270).

## Verified deployments

### Sepolia source

| Contract | Address | Deployment |
| --- | --- | --- |
| DemoSourceWallet | [`0xAA449Fb72Fc8B4989acB5287787660Af5d19C5E8`](https://sepolia.etherscan.io/address/0xAA449Fb72Fc8B4989acB5287787660Af5d19C5E8#code) | [`0x5ba6…ccc9`](https://sepolia.etherscan.io/tx/0x5ba64af320a9d95f8a77cfe8893f31dabe89aeaa21fe25ed4cceae2c253eccc9) |
| DemoRelics721 | [`0x6c6b4275F4429a2825c3c85a4c4430d23FaBF1D9`](https://sepolia.etherscan.io/address/0x6c6b4275F4429a2825c3c85a4c4430d23FaBF1D9#code) | [`0xfebd…61d`](https://sepolia.etherscan.io/tx/0xfebde03ced1b1fda43ef205ec90783212093dea58c2b296d3a5c10560f6c661d) |
| DemoRelics1155 | [`0x323643Af1A28cF70Ffd31F896c083d15Ba90CeBB`](https://sepolia.etherscan.io/address/0x323643Af1A28cF70Ffd31F896c083d15Ba90CeBB#code) | [`0xcd75…8571`](https://sepolia.etherscan.io/tx/0xcd75f53067f2867911b73b63a4bd0d19834c416c9d81bae64c524d4886198571) |

The seeded histories are [`0x9583…2864`](https://sepolia.etherscan.io/tx/0x95833c24d2e6a8c94469419f661cd9e9e8f500733e8acc07011ec9dc57232864) for ERC-721 and [`0xf780…d6c3`](https://sepolia.etherscan.io/tx/0xf78006cbcd6db50a979af5621932462a30e12ffc7d96d5a856322a54eb93d6c3) for ERC-1155. The source-only ERC-1271 wallet owns ERC-721 token `4` and 11 units of ERC-1155 token `9` at the snapshot boundary; its signed authorization maps both allocations to a destination EOA authority.

### Base Sepolia destination

| Role | Address |
| --- | --- |
| 2-of-3 Safe administrator | [`0x3B71419690766083c576eF109FAab4f4b99B8517`](https://sepolia.basescan.org/address/0x3B71419690766083c576eF109FAab4f4b99B8517) |
| Destination ERC-1271 authority | [`0x0d2848a72fdd385aF3fdaC11b9bEaE8E7cb3EE90`](https://sepolia.basescan.org/address/0x0d2848a72fdd385aF3fdaC11b9bEaE8E7cb3EE90#code) |
| ERC721MigrationClaim | [`0x422FE0B8f0E2e1381F04F920840c88F9A0718ae5`](https://sepolia.basescan.org/address/0x422FE0B8f0E2e1381F04F920840c88F9A0718ae5#code) |
| MigratedERC721 | [`0x417513ddD2087C93B4414718ED803781A81D25b7`](https://sepolia.basescan.org/address/0x417513ddD2087C93B4414718ED803781A81D25b7#code) |
| ERC1155MigrationClaim | [`0xe5325f772402ff0FF1a17fF984dd813891ac7137`](https://sepolia.basescan.org/address/0xe5325f772402ff0FF1a17fF984dd813891ac7137#code) |
| MigratedERC1155 | [`0x416986df24d1391Ef311C93713F2d242E7c4829d`](https://sepolia.basescan.org/address/0x416986df24d1391Ef311C93713F2d242E7c4829d#code) |

The claim window is `2026-08-21 06:07:58 UTC` through `2026-09-20 06:07:58 UTC`. Two independent Base Sepolia RPC clients returned the same bytecode, campaign immutables, root/version/digest, token minter and lock, owner/pending owner, window, and pause state before launch. Both tokens are owned by the Safe, have zero pending owner, and have their minter permanently locked to the corresponding typed claim contract.

Administrative transactions:

- Accept ERC-721 token ownership: [`0xb2fa…fb75`](https://sepolia.basescan.org/tx/0xb2fabed21951d85664145fec31768363b81f8966e45b8c0cb954eaa3c41efb75)
- Accept ERC-1155 token ownership: [`0x86b3…177`](https://sepolia.basescan.org/tx/0x86b3acadffe351d6f419bdbed93eca524fa547a6cf829fa5254bf54da56ed177)
- Register ERC-721 root v1: [`0xc89e…ac82`](https://sepolia.basescan.org/tx/0xc89e3aff41eeb2e872ad02ce6dda59610a975fd4efe31e69b5bb6fb9af2cac82)
- Register ERC-1155 root v1: [`0xb676…1b77`](https://sepolia.basescan.org/tx/0xb676ef0462ed12e65b63191d86fb5e1f15ba0ba78bd562e6535493bb257a1b77)
- Register corrected ERC-1155 root v2: [`0xa577…2270`](https://sepolia.basescan.org/tx/0xa577d320022fe581c6bda8ea7095acf9b9c63b069bb3a9f89559d5f637da2270)
- Pause ERC-721 campaign: [`0xdb9b…9f10`](https://sepolia.basescan.org/tx/0xdb9bd83f00ec752bd27a4618e9b7af076ed62e4cf402e407b0cfde0449479f10)
- Resume ERC-721 campaign: [`0x9ce3…1a0b`](https://sepolia.basescan.org/tx/0x9ce3e8e88badbe2b6bae26b98808fcbf6961e16d915a2f42601e6d1b38a51a0b)

While paused, a valid claim simulation reverted with `EnforcedPause()` (`0xd93c0665`). After resumption but before `claimStart`, it reverted with `ClaimWindowClosed` (`0x6b7427ce`).

## Claim matrix

| Evidence | Campaign/path | Transaction |
| --- | --- | --- |
| ERC-721 multiproof batch | Owner A, two leaves | [`0x2598…43ce`](https://sepolia.basescan.org/tx/0x25980d0d06574ae28c5b3ffe3bfd54da0752164a2f9052023dbbe85e52ad43ce) |
| Destination ERC-1271 delegated claim | Owner B signature, independent relayer | [`0x4ee7…1fbb`](https://sepolia.basescan.org/tx/0x4ee7adeb0be9ea38461814f0f9415f8eb063c1648c9eb75b0bd161239be61fbb) |
| Source-only ERC-1271 authorization + direct claim | Source wallet → Owner C authority | [`0xe629…bede`](https://sepolia.basescan.org/tx/0xe6292cb632b5df671592671e632cd7b3e8a1a9c6a1823c823925604c3849bede) |
| ERC-1155 direct claim | Owner A | [`0x3319…d057`](https://sepolia.basescan.org/tx/0x331984acc98ef5bc5322fdb80d987e8923b7185db40ca598a02fb0cbc9b2d057) |
| ERC-1155 multiproof batch | Owner B, two leaves | [`0x5d21…0790`](https://sepolia.basescan.org/tx/0x5d21b0b0d97d19ab92a169a9f1ead26c450b11b2f4f901953aea9b6fb1ea0790) |
| Delegated EOA + source-only ERC-1271 authorization | Owner C signature, independent relayer | [`0x551c…63c3`](https://sepolia.basescan.org/tx/0x551cfd3f011568796c9028a033ed6eb90b5f49055856a2a9bcea1ae85b2f63c3) |

Read-only failure-path simulations at launch also proved:

| Failure | Expected custom error |
| --- | --- |
| Proof from a different leaf | `InvalidProof()` (`0x09bde339`) |
| Reusing a claimed leaf | `AlreadyClaimed(uint256)` (`0xb3167bfa`) |
| Expired delegated authorization | `ExpiredSignature(uint256,uint256)` (`0xdba17e9a`) |
| Wrong signer for destination ERC-1271 authority | `InvalidSignature(address)` (`0xd855c4f4`) |
| Root change after launch/first claim | `RootFrozen()` (`0x9bce7758`) |

## Final reconciliation

After the six successful claim transactions:

- `claimedCount()` is `4` for each campaign and every leaf index `0..3` is set.
- ERC-721 token IDs `1` and `2` belong to Owner A, token `3` belongs to Owner B, and token `4` belongs to Owner C.
- ERC-1155 balances are Owner A: token `7` amount `3`; Owner B: token `7` amount `5` and token `8` amount `2`; Owner C: token `9` amount `11`.
- The same final state was read through the secondary Base Sepolia provider.
- ERC-721 metadata was frozen by the Safe in [`0xc510…d4ff`](https://sepolia.basescan.org/tx/0xc510e809abf9b73bdf345f3814821a0d25b297854333e97b579cf6aef1bbd4ff).
- ERC-1155 metadata was frozen by the Safe in [`0x83be…7bc7`](https://sepolia.basescan.org/tx/0x83be423d5595a48100bd64edf01fbaffadbd3100bbabb3c96fb6f6dafdc77bc7).

The [live application](https://web-production-fab71.up.railway.app/) serves the reviewed ERC-1155 v2 bundle and independently checks the contract code, full campaign domain, root/version/digest, source boundary, token/minter lock, Safe ownership, local proofs, pause state, and claim window. Any disagreement disables claims and signing while leaving the evidence readable.
