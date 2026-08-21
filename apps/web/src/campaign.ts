import {
  concatHex, encodeAbiParameters, isAddress, isHex, keccak256, toBytes,
  type Address, type Hex,
} from "viem";

const artifactNames = ["manifest.json", "proofs.json", "status.json", "artifact-digests.json"] as const;

export type ManifestEntry = {
  standard: 1 | 2; tokenId: string; amount: string; sourceOwner: Address;
  claimAuthority: Address; destinationRecipient: Address; leafIndex: string; leafHash: Hex;
};
export type Manifest = {
  format: "evm-migration-manifest-v2";
  campaign: {
    migrationId: Hex; sourceChainId: string; sourceContract: Address; snapshotBlock: string;
    snapshotBlockHash: Hex; destinationChainId: string; standard: 1 | 2;
    finalityPolicy: string; leafEncoding: string[];
  };
  entries: ManifestEntry[];
};
export type ProofBundle = {
  root: Hex;
  singleProofs: { leafIndex: string; proof: Hex[] }[];
  ownerMultiProofs: { claimAuthority: Address; leafIndices: string[]; proof: Hex[]; proofFlags: boolean[] }[];
};
export type StatusArtifact = {
  environment: string; chainId: string; live: boolean; generatedAt: string;
  snapshotBlock: string; snapshotBlockHash: Hex; manifestEntries: string; merkleRoot: Hex;
  claimsCompleted: string; reconciliationStatus: string; lastVerifiedCommit: string;
};
export type ArtifactDigests = {
  format: "evm-migration-artifact-digests-v1"; files: Record<string, Hex>; bundleDigest: Hex;
  cliVersion: string; verifiedCommit: string; sourceBlock: string; sourceBlockHash: Hex;
};
export type CampaignArtifacts = {
  manifest: Manifest; proofs: ProofBundle; status: StatusArtifact; digests: ArtifactDigests;
};
export type RelayPayload = {
  claim: {
    tokenId: string; amount: string; sourceOwner: Address; claimAuthority: Address;
    destinationRecipient: Address; leafIndex: string;
  };
  proof: Hex[]; merkleRoot: Hex; rootVersion: string; nonce: string;
  deadline: string; signature: Hex;
};

export async function loadCampaign(): Promise<CampaignArtifacts> {
  const responses = await Promise.all(artifactNames.map((name) => fetch(`/campaign/${name}`).then(assertResponse)));
  const raw = await Promise.all(responses.map((response) => response.text()));
  return parseCampaignArtifacts({
    manifest: raw[0], proofs: raw[1], status: raw[2], digests: raw[3],
  });
}

export function parseCampaignArtifacts(raw: {
  manifest: string; proofs: string; status: string; digests: string;
}): CampaignArtifacts {
  const manifest = parseManifest(JSON.parse(raw.manifest));
  const proofs = parseProofs(JSON.parse(raw.proofs));
  const status = parseStatus(JSON.parse(raw.status));
  const digests = parseDigests(JSON.parse(raw.digests));
  const bundlePreimage = Object.entries(digests.files)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, digest]) => `${name}${digest}`)
    .join("");
  if (keccak256(toBytes(bundlePreimage)) !== digests.bundleDigest) {
    throw new Error("Artifact bundle digest is invalid");
  }
  const content = [raw.manifest, raw.proofs];
  for (const [index, name] of artifactNames.slice(0, 2).entries()) {
    if (digests.files[name] !== keccak256(toBytes(content[index]!))) {
      throw new Error(`${name} does not match artifact-digests.json`);
    }
  }
  if (proofs.root !== status.merkleRoot) throw new Error("Status and proof roots differ");
  if (
    status.chainId !== manifest.campaign.destinationChainId
    || status.snapshotBlock !== manifest.campaign.snapshotBlock
    || status.snapshotBlockHash !== manifest.campaign.snapshotBlockHash
    || status.manifestEntries !== manifest.entries.length.toString()
    || BigInt(status.claimsCompleted) > BigInt(status.manifestEntries)
  ) throw new Error("Status and manifest metadata differ");
  if (manifest.entries.some((entry) => hashLeaf(manifest, entry) !== entry.leafHash)) {
    throw new Error("A manifest leaf hash is invalid");
  }
  const proofByLeaf = new Map(proofs.singleProofs.map((proof) => [proof.leafIndex, proof.proof]));
  if (manifest.entries.some((entry) => {
    const proof = proofByLeaf.get(entry.leafIndex);
    return !proof || !verifyProof(entry.leafHash, proof, proofs.root);
  })) throw new Error("A published Merkle proof is invalid");
  return { manifest, proofs, status, digests };
}

export function toClaimData(entry: ManifestEntry) {
  return {
    tokenId: BigInt(entry.tokenId), amount: BigInt(entry.amount), sourceOwner: entry.sourceOwner,
    claimAuthority: entry.claimAuthority, destinationRecipient: entry.destinationRecipient,
    leafIndex: BigInt(entry.leafIndex),
  };
}

export function parseRelayPayload(value: string): RelayPayload {
  const payload = record(JSON.parse(value), "payload");
  const claim = record(payload.claim, "claim");
  return {
    claim: {
      tokenId: decimal(claim.tokenId, "claim.tokenId"),
      amount: positiveDecimal(claim.amount, "claim.amount"),
      sourceOwner: address(claim.sourceOwner, "claim.sourceOwner"),
      claimAuthority: address(claim.claimAuthority, "claim.claimAuthority"),
      destinationRecipient: address(claim.destinationRecipient, "claim.destinationRecipient"),
      leafIndex: decimal(claim.leafIndex, "claim.leafIndex"),
    },
    proof: proofArray(payload.proof, "proof"),
    merkleRoot: bytes32(payload.merkleRoot, "merkleRoot"),
    rootVersion: decimal(payload.rootVersion, "rootVersion"),
    nonce: decimal(payload.nonce, "nonce"),
    deadline: decimal(payload.deadline, "deadline"),
    signature: hex(payload.signature, "signature"),
  };
}

export function hashLeaf(manifest: Manifest, entry: ManifestEntry): Hex {
  const campaign = manifest.campaign;
  const inner = keccak256(encodeAbiParameters(
    [
      { type: "bytes32" }, { type: "uint256" }, { type: "address" }, { type: "uint256" },
      { type: "bytes32" }, { type: "uint256" }, { type: "uint8" }, { type: "uint256" },
      { type: "uint256" }, { type: "address" }, { type: "address" }, { type: "address" },
      { type: "uint256" },
    ],
    [
      campaign.migrationId, BigInt(campaign.sourceChainId), campaign.sourceContract,
      BigInt(campaign.snapshotBlock), campaign.snapshotBlockHash,
      BigInt(campaign.destinationChainId), entry.standard, BigInt(entry.tokenId),
      BigInt(entry.amount), entry.sourceOwner, entry.claimAuthority, entry.destinationRecipient,
      BigInt(entry.leafIndex),
    ],
  ));
  return keccak256(inner);
}

export function verifyProof(leaf: Hex, proof: Hex[], root: Hex): boolean {
  return proof.reduce((hash, sibling) => keccak256(
    hash.toLowerCase() < sibling.toLowerCase()
      ? concatHex([hash, sibling]) : concatHex([sibling, hash]),
  ), leaf) === root;
}

function parseManifest(value: unknown): Manifest {
  const input = record(value, "manifest");
  if (input.format !== "evm-migration-manifest-v2") throw new Error("Unsupported manifest format");
  const campaign = record(input.campaign, "campaign");
  const standard = tokenStandard(campaign.standard, "campaign.standard");
  if (!Array.isArray(input.entries)) throw new Error("manifest.entries must be an array");
  const entries = input.entries.map((item, index) => {
    const entry = record(item, `entries[${index}]`);
    const parsed: ManifestEntry = {
      standard: tokenStandard(entry.standard, `entries[${index}].standard`),
      tokenId: decimal(entry.tokenId, `entries[${index}].tokenId`),
      amount: positiveDecimal(entry.amount, `entries[${index}].amount`),
      sourceOwner: address(entry.sourceOwner, `entries[${index}].sourceOwner`),
      claimAuthority: address(entry.claimAuthority, `entries[${index}].claimAuthority`),
      destinationRecipient: address(entry.destinationRecipient, `entries[${index}].destinationRecipient`),
      leafIndex: decimal(entry.leafIndex, `entries[${index}].leafIndex`),
      leafHash: bytes32(entry.leafHash, `entries[${index}].leafHash`),
    };
    if (parsed.standard !== standard) throw new Error(`entries[${index}] standard differs from campaign`);
    if (standard === 1 && parsed.amount !== "1") throw new Error("ERC-721 amount must be 1");
    return parsed;
  });
  if (new Set(entries.map((entry) => entry.leafIndex)).size !== entries.length) {
    throw new Error("Manifest leaf indices are not unique");
  }
  return {
    format: input.format,
    campaign: {
      migrationId: bytes32(campaign.migrationId, "campaign.migrationId"),
      sourceChainId: decimal(campaign.sourceChainId, "campaign.sourceChainId"),
      sourceContract: address(campaign.sourceContract, "campaign.sourceContract"),
      snapshotBlock: decimal(campaign.snapshotBlock, "campaign.snapshotBlock"),
      snapshotBlockHash: bytes32(campaign.snapshotBlockHash, "campaign.snapshotBlockHash"),
      destinationChainId: decimal(campaign.destinationChainId, "campaign.destinationChainId"),
      standard,
      finalityPolicy: text(campaign.finalityPolicy, "campaign.finalityPolicy"),
      leafEncoding: stringArray(campaign.leafEncoding, "campaign.leafEncoding"),
    }, entries,
  };
}

function parseProofs(value: unknown): ProofBundle {
  const input = record(value, "proofs");
  if (!Array.isArray(input.singleProofs) || !Array.isArray(input.ownerMultiProofs)) {
    throw new Error("Proof collections must be arrays");
  }
  return {
    root: bytes32(input.root, "proofs.root"),
    singleProofs: input.singleProofs.map((item, index) => {
      const proof = record(item, `singleProofs[${index}]`);
      return { leafIndex: decimal(proof.leafIndex, `singleProofs[${index}].leafIndex`), proof: proofArray(proof.proof, `singleProofs[${index}].proof`) };
    }),
    ownerMultiProofs: input.ownerMultiProofs.map((item, index) => {
      const proof = record(item, `ownerMultiProofs[${index}]`);
      if (!Array.isArray(proof.leafIndices) || !Array.isArray(proof.proofFlags)) throw new Error(`ownerMultiProofs[${index}] is malformed`);
      return {
        claimAuthority: address(proof.claimAuthority, `ownerMultiProofs[${index}].claimAuthority`),
        leafIndices: proof.leafIndices.map((leaf, leafIndex) => decimal(leaf, `leafIndices[${leafIndex}]`)),
        proof: proofArray(proof.proof, `ownerMultiProofs[${index}].proof`),
        proofFlags: proof.proofFlags.map((flag) => {
          if (typeof flag !== "boolean") throw new Error("proof flag must be boolean");
          return flag;
        }),
      };
    }),
  };
}

function parseStatus(value: unknown): StatusArtifact {
  const input = record(value, "status");
  if (typeof input.live !== "boolean") throw new Error("status.live must be boolean");
  return {
    environment: text(input.environment, "status.environment"),
    chainId: decimal(input.chainId, "status.chainId"), live: input.live,
    generatedAt: decimal(input.generatedAt, "status.generatedAt"),
    snapshotBlock: decimal(input.snapshotBlock, "status.snapshotBlock"),
    snapshotBlockHash: bytes32(input.snapshotBlockHash, "status.snapshotBlockHash"),
    manifestEntries: decimal(input.manifestEntries, "status.manifestEntries"),
    merkleRoot: bytes32(input.merkleRoot, "status.merkleRoot"),
    claimsCompleted: decimal(input.claimsCompleted, "status.claimsCompleted"),
    reconciliationStatus: text(input.reconciliationStatus, "status.reconciliationStatus"),
    lastVerifiedCommit: text(input.lastVerifiedCommit, "status.lastVerifiedCommit"),
  };
}

function parseDigests(value: unknown): ArtifactDigests {
  const input = record(value, "artifact digests");
  if (input.format !== "evm-migration-artifact-digests-v1") throw new Error("Unsupported digest format");
  const files = record(input.files, "artifact digests files");
  return {
    format: input.format,
    files: Object.fromEntries(Object.entries(files).map(([name, digest]) => [name, bytes32(digest, `files.${name}`)])),
    bundleDigest: bytes32(input.bundleDigest, "bundleDigest"),
    cliVersion: text(input.cliVersion, "cliVersion"),
    verifiedCommit: text(input.verifiedCommit, "verifiedCommit"),
    sourceBlock: decimal(input.sourceBlock, "sourceBlock"),
    sourceBlockHash: bytes32(input.sourceBlockHash, "sourceBlockHash"),
  };
}

function record(value: unknown, field: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${field} must be an object`);
  return value as Record<string, unknown>;
}
function decimal(value: unknown, field: string): string {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be a decimal string`);
  return value;
}
function positiveDecimal(value: unknown, field: string): string {
  const parsed = decimal(value, field); if (parsed === "0") throw new Error(`${field} must be positive`); return parsed;
}
function address(value: unknown, field: string): Address {
  if (typeof value !== "string" || !isAddress(value, { strict: true })) throw new Error(`${field} must be an address`); return value;
}
function bytes32(value: unknown, field: string): Hex {
  if (typeof value !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(value)) throw new Error(`${field} must be 32 bytes`); return value as Hex;
}
function hex(value: unknown, field: string): Hex {
  if (typeof value !== "string" || !isHex(value, { strict: true })) throw new Error(`${field} must be hex`); return value;
}
function proofArray(value: unknown, field: string): Hex[] {
  if (!Array.isArray(value)) throw new Error(`${field} must be an array`); return value.map((item, index) => bytes32(item, `${field}[${index}]`));
}
function tokenStandard(value: unknown, field: string): 1 | 2 {
  if (value !== 1 && value !== 2) throw new Error(`${field} must be 1 or 2`); return value;
}
function text(value: unknown, field: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${field} must be text`); return value;
}
function stringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) throw new Error(`${field} must be a string array`); return value as string[];
}
function assertResponse(response: Response): Response {
  if (!response.ok) throw new Error(`Campaign artifact unavailable (${response.status})`); return response;
}
