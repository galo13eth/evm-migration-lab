import { isAddress, isHex, type Address, type Hex } from "viem";

export type ManifestEntry = {
  standard: number;
  tokenId: string;
  amount: string;
  sourceOwner: Address;
  destinationRecipient: Address;
  leafIndex: number;
  leafHash: Hex;
};

export type Manifest = {
  format: string;
  campaign: {
    migrationId: Hex;
    sourceChainId: number;
    sourceContract: Address;
    snapshotBlock: number;
    snapshotBlockHash: Hex;
    standard: number;
  };
  entries: ManifestEntry[];
};

export type ProofBundle = {
  root: Hex;
  singleProofs: { leafIndex: number; proof: Hex[] }[];
  ownerMultiProofs: {
    sourceOwner: Address;
    leafIndices: number[];
    proof: Hex[];
    proofFlags: boolean[];
  }[];
};

export type StatusArtifact = {
  snapshotBlock: number;
  manifestEntries: number;
  merkleRoot: Hex;
  claimsCompleted: number;
  reconciliationStatus: string;
  lastVerifiedCommit: string;
};

export type CampaignArtifacts = {
  manifest: Manifest;
  proofs: ProofBundle;
  status: StatusArtifact;
};

export type RelayPayload = {
  claim: {
    standard: number;
    tokenId: string;
    amount: string;
    sourceOwner: Address;
    recipient: Address;
    leafIndex: string;
  };
  proof: Hex[];
  nonce: string;
  deadline: string;
  signature: Hex;
};

export async function loadCampaign(): Promise<CampaignArtifacts> {
  const [manifest, proofs, status] = await Promise.all([
    fetch("/campaign/manifest.json").then(assertResponse),
    fetch("/campaign/proofs.json").then(assertResponse),
    fetch("/campaign/status.json").then(assertResponse),
  ]);
  return {
    manifest: (await manifest.json()) as Manifest,
    proofs: (await proofs.json()) as ProofBundle,
    status: (await status.json()) as StatusArtifact,
  };
}

function assertResponse(response: Response): Response {
  if (!response.ok) throw new Error(`Campaign artifact unavailable (${response.status})`);
  return response;
}

export function toClaimData(entry: ManifestEntry) {
  return {
    standard: entry.standard,
    tokenId: BigInt(entry.tokenId),
    amount: BigInt(entry.amount),
    sourceOwner: entry.sourceOwner,
    recipient: entry.destinationRecipient,
    leafIndex: BigInt(entry.leafIndex),
  };
}

export function parseRelayPayload(value: string): RelayPayload {
  const payload = JSON.parse(value) as Partial<RelayPayload>;
  const claim = payload.claim;
  if (
    !claim ||
    (claim.standard !== 1 && claim.standard !== 2) ||
    !isAddress(claim.sourceOwner ?? "") ||
    !isAddress(claim.recipient ?? "") ||
    !payload.signature ||
    !isHex(payload.signature) ||
    !Array.isArray(payload.proof) ||
    !payload.proof.every((item) => isHex(item, { strict: true }))
  ) {
    throw new Error("Payload fields are malformed");
  }
  for (const amount of [claim.tokenId, claim.amount, claim.leafIndex, payload.nonce, payload.deadline]) {
    if (amount === undefined || BigInt(amount) < 0n) throw new Error("Numeric fields are malformed");
  }
  if (claim.standard === 1 && claim.amount !== "1") throw new Error("ERC-721 amount must be 1");
  if (claim.standard === 2 && claim.amount === "0") throw new Error("ERC-1155 amount must be positive");
  return payload as RelayPayload;
}
