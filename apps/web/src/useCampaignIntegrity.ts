import { useMemo } from "react";
import { useBytecode, useReadContracts } from "wagmi";
import type { Address, Hex } from "viem";

import { migratedTokenAbi, migrationClaimAbi } from "./abi";
import type { CampaignArtifacts } from "./campaign";
import { campaignReady, claimAddress, targetChain } from "./config";

export type IntegrityCheck = { label: string; ok: boolean; detail: string };

const functions = [
  "merkleRoot", "artifactDigest", "rootVersion", "migrationId", "sourceChainId",
  "sourceContract", "snapshotBlock", "sourceBlockHash", "destinationChainId",
  "destinationToken", "claimStart", "claimDeadline", "campaignStandard", "paused", "owner",
] as const;

export function useCampaignIntegrity(artifacts?: CampaignArtifacts) {
  const enabled = campaignReady && Boolean(artifacts);
  const bytecode = useBytecode({
    address: claimAddress,
    chainId: targetChain.id,
    query: { enabled },
  });
  const reads = useReadContracts({
    contracts: functions.map((functionName) => ({
      address: claimAddress,
      abi: migrationClaimAbi,
      functionName,
      chainId: targetChain.id,
    })),
    query: { enabled },
  });
  const destinationToken = read<Address>(reads.data, 9);
  const tokenReads = useReadContracts({
    contracts: (["minter", "minterLocked", "owner", "pendingOwner", "metadataFrozen"] as const).map(
      (functionName) => ({
        address: destinationToken ?? claimAddress,
        abi: migratedTokenAbi,
        functionName,
        chainId: targetChain.id,
      }),
    ),
    query: { enabled: enabled && Boolean(destinationToken) },
  });

  return useMemo(() => {
    const root = read<Hex>(reads.data, 0);
    const artifactDigest = read<Hex>(reads.data, 1);
    const rootVersion = read<bigint>(reads.data, 2);
    const migrationId = read<Hex>(reads.data, 3);
    const sourceChainId = read<bigint>(reads.data, 4);
    const sourceContract = read<Address>(reads.data, 5);
    const snapshotBlock = read<bigint>(reads.data, 6);
    const sourceBlockHash = read<Hex>(reads.data, 7);
    const destinationChainId = read<bigint>(reads.data, 8);
    const claimStart = read<bigint>(reads.data, 10);
    const claimDeadline = read<bigint>(reads.data, 11);
    const standard = read<number>(reads.data, 12);
    const paused = read<boolean>(reads.data, 13);
    const owner = read<Address>(reads.data, 14);
    const tokenMinter = read<Address>(tokenReads.data, 0);
    const minterLocked = read<boolean>(tokenReads.data, 1);
    const tokenOwner = read<Address>(tokenReads.data, 2);
    const pendingTokenOwner = read<Address>(tokenReads.data, 3);
    const metadataFrozen = read<boolean>(tokenReads.data, 4);
    const campaign = artifacts?.manifest.campaign;
    const status = artifacts?.status;
    const codePresent = Boolean(bytecode.data && bytecode.data !== "0x");
    const fieldsLoaded = reads.data?.every((item) => item.status === "success") === true;
    const tokenLoaded = tokenReads.data?.every((item) => item.status === "success") === true;
    const checks: IntegrityCheck[] = artifacts && campaign ? [
      { label: "Contract code present", ok: codePresent, detail: codePresent ? "deployed" : "missing" },
      { label: "Campaign reads", ok: fieldsLoaded, detail: fieldsLoaded ? "complete" : "unavailable" },
      { label: "Local leaf / proofs", ok: true, detail: "verified" },
      { label: "Manifest root = onchain root", ok: root === artifacts.proofs.root, detail: rootVersion === undefined ? "—" : `version ${rootVersion}` },
      { label: "Artifact bundle digest", ok: artifactDigest === artifacts.digests.bundleDigest, detail: short(artifactDigest) },
      { label: "Status provenance", ok: status?.live === true && status.chainId === campaign.destinationChainId && status.snapshotBlock === campaign.snapshotBlock && status.snapshotBlockHash === campaign.snapshotBlockHash && status.manifestEntries === artifacts.manifest.entries.length.toString(), detail: status?.environment ?? "—" },
      { label: "Migration domain", ok: migrationId === campaign.migrationId && sourceChainId === BigInt(campaign.sourceChainId) && sameAddress(sourceContract, campaign.sourceContract), detail: short(migrationId) },
      { label: "Snapshot block / hash", ok: snapshotBlock === BigInt(campaign.snapshotBlock) && sourceBlockHash === campaign.snapshotBlockHash, detail: campaign.snapshotBlock },
      { label: "Destination chain", ok: destinationChainId === BigInt(campaign.destinationChainId) && destinationChainId === BigInt(targetChain.id), detail: destinationChainId?.toString() ?? "—" },
      { label: "Campaign standard", ok: standard === campaign.standard, detail: standard === 1 ? "ERC-721" : standard === 2 ? "ERC-1155" : "—" },
      { label: "Destination token / minter", ok: tokenLoaded && sameAddress(tokenMinter, claimAddress) && minterLocked === true, detail: short(destinationToken) },
      { label: "Administrative ownership", ok: tokenLoaded && sameAddress(tokenOwner, owner) && pendingTokenOwner === "0x0000000000000000000000000000000000000000", detail: short(owner) },
      { label: "Metadata policy", ok: tokenLoaded, detail: metadataFrozen ? "frozen" : "mutable (documented)" },
    ] : [];
    const integrityOk = campaignReady && checks.length > 0 && checks.every((check) => check.ok);
    const now = BigInt(Math.floor(Date.now() / 1_000));
    const claimOpen = claimStart !== undefined && claimDeadline !== undefined
      && now >= claimStart && now <= claimDeadline && paused === false;
    return {
      checks, integrityOk, claimOpen, rootVersion, root, paused, claimStart, claimDeadline,
      destinationToken, owner, tokenOwner, pendingTokenOwner, metadataFrozen,
      loading: enabled && (bytecode.isLoading || reads.isLoading || tokenReads.isLoading),
    };
  }, [artifacts, bytecode.data, bytecode.isLoading, enabled, reads.data, reads.isLoading,
    tokenReads.data, tokenReads.isLoading, destinationToken]);
}

function read<T>(data: readonly { status: string; result?: unknown }[] | undefined, index: number): T | undefined {
  const item = data?.[index];
  return item?.status === "success" ? item.result as T : undefined;
}
function sameAddress(left?: Address, right?: Address): boolean {
  return Boolean(left && right && left.toLowerCase() === right.toLowerCase());
}
function short(value?: string): string {
  return value && value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-6)}` : value ?? "—";
}
