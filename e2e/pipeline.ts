import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  createPublicClient,
  createWalletClient,
  encodeFunctionData,
  http,
  keccak256,
  toHex,
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
  claimAuthority: Address;
  destinationRecipient: Address;
  leafIndex: string;
  leafHash: Hex;
};
type Manifest = {
  campaign: {
    migrationId: Hex;
    sourceChainId: string;
    sourceContract: Address;
    snapshotBlock: string;
    snapshotBlockHash: Hex;
    destinationChainId: string;
    standard: number;
  };
  entries: ManifestEntry[];
};
type ArtifactDigests = { bundleDigest: Hex };
type Authorization = {
  sourceOwner: Address;
  claimAuthority: Address;
  destinationRecipient: Address;
  signature: Hex;
};
type ProofBundle = {
  root: Hex;
  singleProofs: { leafIndex: string; proof: Hex[] }[];
  ownerMultiProofs: {
    claimAuthority: Address;
    leafIndices: string[];
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
  if (receipt.status !== "success") {
    await publicClient.call({
      account: parameters.account as Address,
      to: parameters.address as Address,
      data: encodeFunctionData(parameters as Parameters<typeof encodeFunctionData>[0]),
    });
  }
  assert.equal(receipt.status, "success");
}

function snapshot(
  standard: "erc721" | "erc1155",
  contract: Address,
  block: bigint,
  migrationId: Hex,
  authorization?: Authorization,
): { manifest: Manifest; proofs: ProofBundle; artifactDigest: Hex; bundle: string } {
  const output = join(state, standard);
  mkdirSync(output, { recursive: true });
  const authorizationPath = join(output, "authorizations.json");
  if (authorization) {
    writeFileSync(
      authorizationPath,
      `${JSON.stringify({
        format: "evm-migration-authorizations-v1",
        authorizations: [authorization],
      }, null, 2)}\n`,
    );
  }
  const authorizationArgs = authorization ? ["--authorization-file", authorizationPath] : [];
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
      "--destination-chain-id",
      destinationChain.id.toString(),
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
      ...authorizationArgs,
    ],
    { cwd: root, env: process.env, stdio: "inherit" },
  );
  const current = JSON.parse(readFileSync(join(output, "current.json"), "utf8")) as {
    path: string;
  };
  const bundle = join(output, current.path);
  return {
    manifest: JSON.parse(readFileSync(join(bundle, "manifest.json"), "utf8")) as Manifest,
    proofs: JSON.parse(readFileSync(join(bundle, "proofs.json"), "utf8")) as ProofBundle,
    artifactDigest: (JSON.parse(
      readFileSync(join(bundle, "artifact-digests.json"), "utf8"),
    ) as ArtifactDigests).bundleDigest,
    bundle,
  };
}

async function authorizeSourceWallet(
  sourceOwner: Address,
  sourceContract: Address,
  snapshotBlock: bigint,
  sourceBlockHash: Hex,
  migrationId: Hex,
): Promise<Authorization> {
  const signature = await wallet(sourceRpc, sourceChain, carol).signTypedData({
    account: carol,
    domain: {
      name: "EVM Migration Snapshot Authorization",
      version: "1",
      chainId: sourceChain.id,
      verifyingContract: sourceContract,
    },
    types: {
      MigrationAuthorization: [
        { name: "migrationId", type: "bytes32" },
        { name: "sourceChainId", type: "uint256" },
        { name: "sourceContract", type: "address" },
        { name: "snapshotBlock", type: "uint256" },
        { name: "sourceBlockHash", type: "bytes32" },
        { name: "destinationChainId", type: "uint256" },
        { name: "claimAuthority", type: "address" },
        { name: "destinationRecipient", type: "address" },
      ],
    },
    primaryType: "MigrationAuthorization",
    message: {
      migrationId,
      sourceChainId: BigInt(sourceChain.id),
      sourceContract,
      snapshotBlock,
      sourceBlockHash,
      destinationChainId: BigInt(destinationChain.id),
      claimAuthority: carol,
      destinationRecipient: carol,
    },
  });
  return { sourceOwner, claimAuthority: carol, destinationRecipient: carol, signature };
}

function claimData(entry: ManifestEntry) {
  return {
    tokenId: BigInt(entry.tokenId),
    amount: BigInt(entry.amount),
    sourceOwner: entry.sourceOwner,
    claimAuthority: entry.claimAuthority,
    destinationRecipient: entry.destinationRecipient,
    leafIndex: BigInt(entry.leafIndex),
  };
}

async function deployCampaign(snapshotData: {
  manifest: Manifest;
  proofs: ProofBundle;
  artifactDigest: Hex;
}) {
  const token721Artifact = artifact("MigratedERC721.sol/MigratedERC721.json");
  const token1155Artifact = artifact("MigratedERC1155.sol/MigratedERC1155.json");
  const standard = snapshotData.manifest.campaign.standard;
  const claimArtifact = artifact(
    standard === 1
      ? "ERC721MigrationClaim.sol/ERC721MigrationClaim.json"
      : "ERC1155MigrationClaim.sol/ERC1155MigrationClaim.json",
  );
  const tokenArtifact = standard === 1 ? token721Artifact : token1155Artifact;
  const token = await deploy(
    destinationWallet,
    destinationPublic,
    tokenArtifact,
    standard === 1
      ? ["Migrated Demo Relics", "mRELIC", "ipfs://migrated-relics/", deployer]
      : ["ipfs://migrated-relics/{id}.json", deployer],
  );
  const latest = await destinationPublic.getBlock();
  const claim = await deploy(destinationWallet, destinationPublic, claimArtifact, [
    snapshotData.manifest.campaign.migrationId,
    BigInt(snapshotData.manifest.campaign.sourceChainId),
    snapshotData.manifest.campaign.sourceContract,
    BigInt(snapshotData.manifest.campaign.snapshotBlock),
    snapshotData.manifest.campaign.snapshotBlockHash,
    BigInt(snapshotData.manifest.campaign.destinationChainId),
    token,
    Number(latest.timestamp + 1n),
    Number(latest.timestamp + 86_400n),
    deployer,
  ]);
  await write(destinationWallet, destinationPublic, {
    address: token,
    abi: tokenArtifact.abi,
    functionName: "setMinter",
    args: [claim],
    account: deployer,
    chain: destinationChain,
  });
  await write(destinationWallet, destinationPublic, {
    address: claim,
    abi: claimArtifact.abi,
    functionName: "setRoot",
    args: [snapshotData.proofs.root, snapshotData.artifactDigest, 1],
    account: deployer,
    chain: destinationChain,
  });
  for (const entry of snapshotData.manifest.entries) {
    const onchainLeaf = await destinationPublic.readContract({
      address: claim,
      abi: claimArtifact.abi,
      functionName: "hashLeaf",
      args: [claimData(entry)],
    });
    assert.equal(onchainLeaf, entry.leafHash, `leaf ${entry.leafIndex} differs across Rust/Solidity`);
  }
  await destinationPublic.request({
    method: "evm_setNextBlockTimestamp",
    params: [toHex(latest.timestamp + 2n)],
  } as never);
  await destinationPublic.request({ method: "evm_mine", params: [] } as never);
  return { claim, claimArtifact, token, tokenArtifact };
}

async function main() {
  const source721Artifact = artifact("DemoRelics721.sol/DemoRelics721.json");
  const source1155Artifact = artifact("DemoRelics1155.sol/DemoRelics1155.json");
  const sourceWalletArtifact = artifact("DemoSourceWallet.sol/DemoSourceWallet.json");
  const sourceOnlyWallet = await deploy(sourceWallet, sourcePublic, sourceWalletArtifact, [carol]);
  const source721 = await deploy(sourceWallet, sourcePublic, source721Artifact, [deployer]);
  const source1155 = await deploy(sourceWallet, sourcePublic, source1155Artifact, [deployer]);
  await write(sourceWallet, sourcePublic, {
    address: source721,
    abi: source721Artifact.abi,
    functionName: "seed",
    args: [[alice, alice, bob, sourceOnlyWallet]],
    account: deployer,
    chain: sourceChain,
  });
  await write(sourceWallet, sourcePublic, {
    address: source1155,
    abi: source1155Artifact.abi,
    functionName: "seed",
    args: [
      [alice, bob, bob, sourceOnlyWallet],
      [7n, 7n, 8n, 9n],
      [3n, 5n, 2n, 11n],
    ],
    account: deployer,
    chain: sourceChain,
  });
  const snapshotBlock = await sourcePublic.getBlockNumber({ cacheTime: 0 });
  const sourceBoundary = await sourcePublic.getBlock({ blockNumber: snapshotBlock });
  const migration721 = keccak256(toBytes("sepolia-base-sepolia-erc721-v1"));
  const migration1155 = keccak256(toBytes("sepolia-base-sepolia-erc1155-v1"));
  assert.throws(
    () => snapshot("erc721", source721, snapshotBlock, migration721),
    "source contract owners must provide an explicit migration authorization",
  );
  const snapshot721 = snapshot(
    "erc721",
    source721,
    snapshotBlock,
    migration721,
    await authorizeSourceWallet(
      sourceOnlyWallet,
      source721,
      snapshotBlock,
      sourceBoundary.hash,
      migration721,
    ),
  );
  const snapshot1155 = snapshot(
    "erc1155",
    source1155,
    snapshotBlock,
    migration1155,
    await authorizeSourceWallet(
      sourceOnlyWallet,
      source1155,
      snapshotBlock,
      sourceBoundary.hash,
      migration1155,
    ),
  );

  const campaign721 = await deployCampaign(snapshot721);
  const aliceProof = snapshot721.proofs.ownerMultiProofs.find(
    (item) => item.claimAuthority.toLowerCase() === alice.toLowerCase(),
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
      address: campaign721.token,
      abi: campaign721.tokenArtifact.abi,
      functionName: "ownerOf",
      args: [tokenId],
    });
    assert.equal((owner as Address).toLowerCase(), alice.toLowerCase());
  }
  const sourceWalletEntry = snapshot721.manifest.entries.find(
    (entry) => entry.sourceOwner.toLowerCase() === sourceOnlyWallet.toLowerCase(),
  );
  assert(sourceWalletEntry, "missing source-only wallet entry");
  const sourceWalletProof = snapshot721.proofs.singleProofs.find(
    (proof) => proof.leafIndex === sourceWalletEntry.leafIndex,
  );
  assert(sourceWalletProof, "missing source-only wallet proof");
  await write(wallet(destinationRpc, destinationChain, carol), destinationPublic, {
    address: campaign721.claim,
    abi: campaign721.claimArtifact.abi,
    functionName: "claim",
    args: [claimData(sourceWalletEntry), sourceWalletProof.proof],
    account: carol,
    chain: destinationChain,
  });
  const sourceWalletTokenOwner = await destinationPublic.readContract({
    address: campaign721.token,
    abi: campaign721.tokenArtifact.abi,
    functionName: "ownerOf",
    args: [4n],
  });
  assert.equal((sourceWalletTokenOwner as Address).toLowerCase(), carol.toLowerCase());

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
      version: "2",
      chainId: destinationChain.id,
      verifyingContract: campaign1155.claim,
    },
    types: {
      DelegatedClaim: [
        { name: "leafHash", type: "bytes32" },
        { name: "merkleRoot", type: "bytes32" },
        { name: "rootVersion", type: "uint64" },
        { name: "destinationRecipient", type: "address" },
        { name: "nonce", type: "uint256" },
        { name: "deadline", type: "uint256" },
      ],
    },
    primaryType: "DelegatedClaim",
    message: {
      leafHash: bobEntry.leafHash,
      merkleRoot: snapshot1155.proofs.root,
      rootVersion: 1n,
      destinationRecipient: bobEntry.destinationRecipient,
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
      address: campaign1155.token,
      abi: campaign1155.tokenArtifact.abi,
      functionName: "balanceOf",
      args: [entry.destinationRecipient, BigInt(entry.tokenId)],
    });
    assert.equal(balance, BigInt(entry.amount));
    const claimed = await destinationPublic.readContract({
      address: campaign1155.claim,
      abi: campaign1155.claimArtifact.abi,
      functionName: "isClaimed",
      args: [BigInt(entry.leafIndex)],
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
  assert.equal(claimed721, 3n);
  assert.equal(claimed1155, 2n);
  const statusPath = join(snapshot1155.bundle, "status.json");
  const status = JSON.parse(readFileSync(statusPath, "utf8")) as Record<string, unknown>;
  status.environment = "local-e2e";
  status.live = false;
  status.claimsCompleted = claimed1155.toString();
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
        migratedERC1155: campaign1155.token,
        artifactBundle: snapshot1155.bundle,
      },
      null,
      2,
    )}\n`,
  );
  console.log("E2E migration pipeline passed: snapshot → root → batch/direct/delegated claims");
}

await main();
