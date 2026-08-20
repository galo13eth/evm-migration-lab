import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  createPublicClient,
  createWalletClient,
  http,
  keccak256,
  toBytes,
  type Abi,
  type Address,
  type Chain,
  type Hex,
  type PublicClient,
  type WalletClient,
} from "viem";
import { defineChain } from "viem/utils";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const state = join(root, "e2e", ".state");
const sourceRpc = process.env.SOURCE_RPC_URL ?? "http://127.0.0.1:8545";
const destinationRpc = process.env.DESTINATION_RPC_URL ?? "http://127.0.0.1:8546";
const cargo = process.env.CARGO ?? "cargo";

const accounts = [
  "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
  "0x70997970c51812dc3a010c7d01b50e0d17dc79c8",
  "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc",
  "0x90f79bf6eb2c4f870365e785982e1f101e93b906",
] as const;
const [deployer, alice, bob, carol] = accounts;

const sourceChain = defineChain({
  id: 31_337,
  name: "Source Anvil",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [sourceRpc] } },
});
const destinationChain = defineChain({
  id: 31_338,
  name: "Destination Anvil",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [destinationRpc] } },
});

type Artifact = { abi: Abi; bytecode: { object: Hex } };
type ManifestEntry = {
  standard: number;
  tokenId: string;
  amount: string;
  sourceOwner: Address;
  destinationRecipient: Address;
  leafIndex: number;
  leafHash: Hex;
};
type Manifest = {
  campaign: {
    migrationId: Hex;
    sourceChainId: number;
    sourceContract: Address;
    snapshotBlock: number;
  };
  entries: ManifestEntry[];
};
type ProofBundle = {
  root: Hex;
  singleProofs: { leafIndex: number; proof: Hex[] }[];
  ownerMultiProofs: {
    sourceOwner: Address;
    leafIndices: number[];
    proof: Hex[];
    proofFlags: boolean[];
  }[];
};

const sourcePublic = createPublicClient({ chain: sourceChain, transport: http(sourceRpc) });
const destinationPublic = createPublicClient({ chain: destinationChain, transport: http(destinationRpc) });
const sourceWallet = wallet(sourceRpc, sourceChain, deployer);
const destinationWallet = wallet(destinationRpc, destinationChain, deployer);

function wallet(
  rpc: string,
  chain: Chain,
  account: Address,
): WalletClient {
  return createWalletClient({ account, chain, transport: http(rpc) });
}

function artifact(relative: string): Artifact {
  return JSON.parse(readFileSync(join(root, "contracts", "out", relative), "utf8")) as Artifact;
}

async function deploy(
  client: WalletClient,
  publicClient: PublicClient,
  item: Artifact,
  args: readonly unknown[],
): Promise<Address> {
  const hash = await client.deployContract({
    abi: item.abi,
    bytecode: item.bytecode.object,
    args,
    account: deployer,
    chain: client.chain,
  });
  const receipt = await publicClient.waitForTransactionReceipt({ hash });
  assert(receipt.contractAddress, "deployment did not return a contract address");
  return receipt.contractAddress;
}

async function write(
  client: WalletClient,
  publicClient: PublicClient,
  parameters: Parameters<WalletClient["writeContract"]>[0],
): Promise<void> {
  const hash = await client.writeContract(parameters);
  const receipt = await publicClient.waitForTransactionReceipt({ hash });
  assert.equal(receipt.status, "success");
}

function snapshot(
  standard: "erc721" | "erc1155",
  contract: Address,
  block: bigint,
  migrationId: Hex,
): { manifest: Manifest; proofs: ProofBundle } {
  const output = join(state, standard);
  mkdirSync(output, { recursive: true });
  execFileSync(
    cargo,
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "evm-snapshot",
      "--",
      "--rpc-url",
      sourceRpc,
      "--contract",
      contract,
      "--standard",
      standard,
      "--snapshot-block",
      block.toString(),
      "--migration-id",
      migrationId,
      "--output",
      output,
      "--chunk-size",
      "2",
      "--concurrency",
      "2",
      "--sample-size",
      "4",
      "--confirmations",
      "0",
    ],
    { cwd: root, env: process.env, stdio: "inherit" },
  );
  return {
    manifest: JSON.parse(readFileSync(join(output, "manifest.json"), "utf8")) as Manifest,
    proofs: JSON.parse(readFileSync(join(output, "proofs.json"), "utf8")) as ProofBundle,
  };
}

function claimData(entry: ManifestEntry) {
  return {
    standard: entry.standard,
    tokenId: BigInt(entry.tokenId),
    amount: BigInt(entry.amount),
    sourceOwner: entry.sourceOwner,
    recipient: entry.destinationRecipient,
    leafIndex: BigInt(entry.leafIndex),
  };
}

async function deployCampaign(snapshotData: { manifest: Manifest; proofs: ProofBundle }) {
  const token721Artifact = artifact("MigratedERC721.sol/MigratedERC721.json");
  const token1155Artifact = artifact("MigratedERC1155.sol/MigratedERC1155.json");
  const claimArtifact = artifact("MigrationClaim.sol/MigrationClaim.json");
  const token721 = await deploy(destinationWallet, destinationPublic, token721Artifact, [
    "Migrated Demo Relics",
    "mRELIC",
    "ipfs://migrated-relics/",
    deployer,
  ]);
  const token1155 = await deploy(destinationWallet, destinationPublic, token1155Artifact, [
    "ipfs://migrated-relics/{id}.json",
    deployer,
  ]);
  const latest = await destinationPublic.getBlock();
  const claim = await deploy(destinationWallet, destinationPublic, claimArtifact, [
    snapshotData.manifest.campaign.migrationId,
    BigInt(snapshotData.manifest.campaign.sourceChainId),
    snapshotData.manifest.campaign.sourceContract,
    BigInt(snapshotData.manifest.campaign.snapshotBlock),
    token721,
    token1155,
    Number(latest.timestamp - 1n),
    Number(latest.timestamp + 86_400n),
    deployer,
  ]);
  for (const [address, abi] of [
    [token721, token721Artifact.abi],
    [token1155, token1155Artifact.abi],
  ] as const) {
    await write(destinationWallet, destinationPublic, {
      address,
      abi,
      functionName: "setMinter",
      args: [claim],
      account: deployer,
      chain: destinationChain,
    });
  }
  await write(destinationWallet, destinationPublic, {
    address: claim,
    abi: claimArtifact.abi,
    functionName: "setRoot",
    args: [snapshotData.proofs.root, 1],
    account: deployer,
    chain: destinationChain,
  });
  return { claim, claimArtifact, token721, token721Artifact, token1155, token1155Artifact };
}

async function main() {
  const source721Artifact = artifact("DemoRelics721.sol/DemoRelics721.json");
  const source1155Artifact = artifact("DemoRelics1155.sol/DemoRelics1155.json");
  const source721 = await deploy(sourceWallet, sourcePublic, source721Artifact, [deployer]);
  const source1155 = await deploy(sourceWallet, sourcePublic, source1155Artifact, [deployer]);
  await write(sourceWallet, sourcePublic, {
    address: source721,
    abi: source721Artifact.abi,
    functionName: "seed",
    args: [[alice, alice, bob, carol]],
    account: deployer,
    chain: sourceChain,
  });
  await write(sourceWallet, sourcePublic, {
    address: source1155,
    abi: source1155Artifact.abi,
    functionName: "seed",
    args: [
      [alice, bob, bob, carol],
      [7n, 7n, 8n, 9n],
      [3n, 5n, 2n, 11n],
    ],
    account: deployer,
    chain: sourceChain,
  });
  const snapshotBlock = await sourcePublic.getBlockNumber();
  const snapshot721 = snapshot(
    "erc721",
    source721,
    snapshotBlock,
    keccak256(toBytes("sepolia-base-sepolia-erc721-v1")),
  );
  const snapshot1155 = snapshot(
    "erc1155",
    source1155,
    snapshotBlock,
    keccak256(toBytes("sepolia-base-sepolia-erc1155-v1")),
  );

  const campaign721 = await deployCampaign(snapshot721);
  const aliceProof = snapshot721.proofs.ownerMultiProofs.find(
    (item) => item.sourceOwner.toLowerCase() === alice.toLowerCase(),
  );
  assert(aliceProof, "missing Alice ERC-721 multiproof");
  const aliceEntries = aliceProof.leafIndices.map((index) => {
    const entry = snapshot721.manifest.entries.find((item) => item.leafIndex === index);
    assert(entry, `missing leaf ${index}`);
    return claimData(entry);
  });
  await write(wallet(destinationRpc, destinationChain, alice), destinationPublic, {
    address: campaign721.claim,
    abi: campaign721.claimArtifact.abi,
    functionName: "claimBatch",
    args: [aliceEntries, aliceProof.proof, aliceProof.proofFlags],
    account: alice,
    chain: destinationChain,
  });
  for (const tokenId of [1n, 2n]) {
    const owner = await destinationPublic.readContract({
      address: campaign721.token721,
      abi: campaign721.token721Artifact.abi,
      functionName: "ownerOf",
      args: [tokenId],
    });
    assert.equal((owner as Address).toLowerCase(), alice.toLowerCase());
  }

  const campaign1155 = await deployCampaign(snapshot1155);
  const aliceEntry = snapshot1155.manifest.entries.find(
    (entry) => entry.sourceOwner.toLowerCase() === alice.toLowerCase(),
  );
  assert(aliceEntry, "missing Alice ERC-1155 entry");
  const aliceSingleProof = snapshot1155.proofs.singleProofs.find(
    (proof) => proof.leafIndex === aliceEntry.leafIndex,
  );
  assert(aliceSingleProof, "missing Alice single proof");
  await write(wallet(destinationRpc, destinationChain, alice), destinationPublic, {
    address: campaign1155.claim,
    abi: campaign1155.claimArtifact.abi,
    functionName: "claim",
    args: [claimData(aliceEntry), aliceSingleProof.proof],
    account: alice,
    chain: destinationChain,
  });

  const bobEntry = snapshot1155.manifest.entries.find(
    (entry) => entry.sourceOwner.toLowerCase() === bob.toLowerCase(),
  );
  assert(bobEntry, "missing Bob ERC-1155 entry");
  const bobProof = snapshot1155.proofs.singleProofs.find(
    (proof) => proof.leafIndex === bobEntry.leafIndex,
  );
  assert(bobProof, "missing Bob single proof");
  const delegatedDeadline = (await destinationPublic.getBlock()).timestamp + 3_600n;
  const signature = await wallet(destinationRpc, destinationChain, bob).signTypedData({
    account: bob,
    domain: {
      name: "EVM Migration Claim",
      version: "1",
      chainId: destinationChain.id,
      verifyingContract: campaign1155.claim,
    },
    types: {
      DelegatedClaim: [
        { name: "leafHash", type: "bytes32" },
        { name: "recipient", type: "address" },
        { name: "nonce", type: "uint256" },
        { name: "deadline", type: "uint256" },
      ],
    },
    primaryType: "DelegatedClaim",
    message: {
      leafHash: bobEntry.leafHash,
      recipient: bobEntry.destinationRecipient,
      nonce: 0n,
      deadline: delegatedDeadline,
    },
  });
  await write(destinationWallet, destinationPublic, {
    address: campaign1155.claim,
    abi: campaign1155.claimArtifact.abi,
    functionName: "claimDelegated",
    args: [claimData(bobEntry), bobProof.proof, 0n, delegatedDeadline, signature],
    account: deployer,
    chain: destinationChain,
  });

  for (const entry of [aliceEntry, bobEntry]) {
    const balance = await destinationPublic.readContract({
      address: campaign1155.token1155,
      abi: campaign1155.token1155Artifact.abi,
      functionName: "balanceOf",
      args: [entry.destinationRecipient, BigInt(entry.tokenId)],
    });
    assert.equal(balance, BigInt(entry.amount));
    const claimed = await destinationPublic.readContract({
      address: campaign1155.claim,
      abi: campaign1155.claimArtifact.abi,
      functionName: "isClaimed",
      args: [1, BigInt(entry.leafIndex)],
    });
    assert.equal(claimed, true);
  }
  const claimed721 = await destinationPublic.readContract({
    address: campaign721.claim,
    abi: campaign721.claimArtifact.abi,
    functionName: "claimedCount",
  });
  const claimed1155 = await destinationPublic.readContract({
    address: campaign1155.claim,
    abi: campaign1155.claimArtifact.abi,
    functionName: "claimedCount",
  });
  assert.equal(claimed721, 2n);
  assert.equal(claimed1155, 2n);
  const statusPath = join(state, "erc1155", "status.json");
  const status = JSON.parse(readFileSync(statusPath, "utf8")) as Record<string, unknown>;
  status.claimsCompleted = Number(claimed1155);
  status.lastVerifiedCommit = process.env.GITHUB_SHA ?? "local-e2e";
  writeFileSync(statusPath, `${JSON.stringify(status, null, 2)}\n`);
  writeFileSync(
    join(state, "deployment.json"),
    `${JSON.stringify(
      {
        sourceRpc,
        destinationRpc,
        chainId: destinationChain.id,
        account: bob,
        claim: campaign1155.claim,
        migratedERC1155: campaign1155.token1155,
      },
      null,
      2,
    )}\n`,
  );
  console.log("E2E migration pipeline passed: snapshot → root → batch/direct/delegated claims");
}

await main();
