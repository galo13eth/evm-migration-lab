import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  useAccount,
  useChainId,
  useReadContract,
  useReadContracts,
  useSignTypedData,
  useSwitchChain,
  useWaitForTransactionReceipt,
  useWriteContract,
} from "wagmi";

import { migrationClaimAbi } from "./abi";
import {
  loadCampaign,
  parseRelayPayload,
  toClaimData,
  type ManifestEntry,
  type RelayPayload,
} from "./campaign";
import { campaignReady, claimAddress, targetChain } from "./config";

type Tab = "claim" | "delegate" | "relay" | "audit";

const tabs: { id: Tab; label: string; index: string }[] = [
  { id: "claim", label: "Eligibility", index: "01" },
  { id: "delegate", label: "Delegate", index: "02" },
  { id: "relay", label: "Relay", index: "03" },
  { id: "audit", label: "Reconcile", index: "04" },
];
const WalletButton = lazy(() => import("./WalletButton"));

export function App() {
  const [tab, setTab] = useState<Tab>("claim");
  const [selectedLeaf, setSelectedLeaf] = useState<number | null>(null);
  const [signedPayload, setSignedPayload] = useState("");
  const [relayInput, setRelayInput] = useState("");
  const [message, setMessage] = useState("");
  const { address, isConnected } = useAccount();
  const chainId = useChainId();
  const { switchChain } = useSwitchChain();
  const queryClient = useQueryClient();
  const campaign = useQuery({ queryKey: ["campaign"], queryFn: loadCampaign });
  const entries = campaign.data?.manifest.entries ?? [];
  const eligible = useMemo(
    () => entries.filter((entry) => entry.sourceOwner.toLowerCase() === address?.toLowerCase()),
    [address, entries],
  );
  const selected = eligible.find((entry) => entry.leafIndex === selectedLeaf) ?? eligible[0];
  const onTargetChain = chainId === targetChain.id;

  const claimedReads = useReadContracts({
    contracts: eligible.map((entry) => ({
      address: claimAddress,
      abi: migrationClaimAbi,
      functionName: "isClaimed" as const,
      args: [1n, BigInt(entry.leafIndex)] as const,
      chainId: targetChain.id,
    })),
    query: { enabled: campaignReady && onTargetChain && eligible.length > 0 },
  });
  const claimedByLeaf = useMemo(
    () =>
      new Map(
        eligible.map((entry, index) => [entry.leafIndex, claimedReads.data?.[index]?.result === true]),
      ),
    [claimedReads.data, eligible],
  );
  const claimedCount = useReadContract({
    address: claimAddress,
    abi: migrationClaimAbi,
    functionName: "claimedCount",
    chainId: targetChain.id,
    query: { enabled: campaignReady && onTargetChain },
  });
  const nonce = useReadContract({
    address: claimAddress,
    abi: migrationClaimAbi,
    functionName: "nonces",
    args: address ? [address] : undefined,
    chainId: targetChain.id,
    query: { enabled: campaignReady && onTargetChain && Boolean(address) },
  });
  const transaction = useWriteContract();
  const receipt = useWaitForTransactionReceipt({ hash: transaction.data });
  const signer = useSignTypedData();

  useEffect(() => {
    if (!receipt.isSuccess) return;
    void claimedReads.refetch();
    void claimedCount.refetch();
    void queryClient.invalidateQueries({ queryKey: ["campaign"] });
  }, [receipt.isSuccess]); // eslint-disable-line react-hooks/exhaustive-deps

  const proofFor = (entry: ManifestEntry) =>
    campaign.data?.proofs.singleProofs.find((proof) => proof.leafIndex === entry.leafIndex);

  async function claimOne(entry: ManifestEntry) {
    const proof = proofFor(entry);
    if (!proof) return setMessage("Proof not found in the published bundle.");
    setMessage("");
    try {
      await transaction.writeContractAsync({
        address: claimAddress,
        abi: migrationClaimAbi,
        functionName: "claim",
        args: [toClaimData(entry), proof.proof],
        chainId: targetChain.id,
      });
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function claimBatch() {
    if (!campaign.data || !address) return;
    const multiproof = campaign.data.proofs.ownerMultiProofs.find(
      (proof) => proof.sourceOwner.toLowerCase() === address.toLowerCase(),
    );
    if (!multiproof) return setMessage("Multiproof not found for this owner.");
    const ordered = multiproof.leafIndices.map((leafIndex) => {
      const entry = eligible.find((item) => item.leafIndex === leafIndex);
      if (!entry) throw new Error(`Manifest entry ${leafIndex} is unavailable`);
      return toClaimData(entry);
    });
    setMessage("");
    try {
      await transaction.writeContractAsync({
        address: claimAddress,
        abi: migrationClaimAbi,
        functionName: "claimBatch",
        args: [ordered, multiproof.proof, multiproof.proofFlags],
        chainId: targetChain.id,
      });
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function signDelegated(entry: ManifestEntry) {
    const proof = proofFor(entry);
    if (!proof || nonce.data === undefined) return setMessage("Proof or nonce is unavailable.");
    const deadline = BigInt(Math.floor(Date.now() / 1_000) + 3_600);
    setMessage("");
    try {
      const signature = await signer.signTypedDataAsync({
        domain: {
          name: "EVM Migration Claim",
          version: "1",
          chainId: targetChain.id,
          verifyingContract: claimAddress,
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
          leafHash: entry.leafHash,
          recipient: entry.destinationRecipient,
          nonce: nonce.data,
          deadline,
        },
      });
      setSignedPayload(
        JSON.stringify(
          {
            claim: {
              ...toClaimData(entry),
              tokenId: entry.tokenId,
              amount: entry.amount,
              leafIndex: String(entry.leafIndex),
            },
            proof: proof.proof,
            nonce: nonce.data.toString(),
            deadline: deadline.toString(),
            signature,
          },
          null,
          2,
        ),
      );
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  async function relay() {
    setMessage("");
    try {
      const payload = parseRelayPayload(relayInput);
      await transaction.writeContractAsync({
        address: claimAddress,
        abi: migrationClaimAbi,
        functionName: "claimDelegated",
        args: [
          {
            ...payload.claim,
            tokenId: BigInt(payload.claim.tokenId),
            amount: BigInt(payload.claim.amount),
            leafIndex: BigInt(payload.claim.leafIndex),
          },
          payload.proof,
          BigInt(payload.nonce),
          BigInt(payload.deadline),
          payload.signature,
        ],
        chainId: targetChain.id,
      });
    } catch (error) {
      setMessage(errorMessage(error));
    }
  }

  const lifecycle = transaction.isPending
    ? "Confirm in wallet"
    : receipt.isLoading
      ? "Transaction pending"
      : receipt.isSuccess
        ? "Confirmed on-chain"
        : "Ready";

  return (
    <div className="site-shell">
      <header className="topbar">
        <a className="brand" href="#top" aria-label="EVM Migration Lab home">
          <span className="brand-mark" aria-hidden="true">M/</span>
          <span>EVM Migration Lab</span>
        </a>
        <div className="header-actions">
          <span className="network-pill"><span className="pulse-dot" />{targetChain.name}</span>
          <Suspense fallback={<span className="wallet-fallback">Connect wallet</span>}>
            <WalletButton />
          </Suspense>
        </div>
      </header>

      <main id="top">
        <section className="hero">
          <div className="hero-copy stagger-1">
            <p className="eyebrow">Snapshot / verify / claim</p>
            <h1>Move the state.<br /><span>Keep the proof.</span></h1>
          </div>
          <p className="hero-note stagger-2">
            A reference migration pipeline for ERC-721 and ERC-1155 state, built to make every
            trust boundary inspectable.
          </p>
        </section>

        <section className="trust-banner stagger-3" aria-label="Important trust notice">
          <span className="warning-glyph" aria-hidden="true">!</span>
          <div><strong>This is not a trustless bridge.</strong><p>The published root commits to a finalized source snapshot. Root administration and manifest generation remain explicit trust assumptions.</p></div>
          <span className="banner-code">POLICY_01</span>
        </section>

        {!campaignReady ? (
          <div className="setup-banner">Artifact preview mode — set <code>VITE_CLAIM_ADDRESS</code> after deployment to enable chain actions.</div>
        ) : null}
        {isConnected && !onTargetChain ? (
          <div className="network-guard">
            <span>Wrong network. Claims execute only on {targetChain.name}.</span>
            <button onClick={() => switchChain({ chainId: targetChain.id })}>Switch network</button>
          </div>
        ) : null}

        <section className="workspace stagger-4">
          <nav className="side-tabs" aria-label="Migration workflow">
            {tabs.map((item) => (
              <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)} aria-current={tab === item.id ? "page" : undefined}>
                <span>{item.index}</span>{item.label}
              </button>
            ))}
          </nav>

          <div className="main-panel">
            {campaign.isLoading ? <PanelState title="Loading campaign" detail="Reading signed-off static artifacts…" /> : null}
            {campaign.isError ? <PanelState title="Artifacts unavailable" detail={errorMessage(campaign.error)} /> : null}
            {campaign.data && tab === "claim" ? (
              <ClaimPanel
                isConnected={isConnected}
                eligible={eligible}
                claimedByLeaf={claimedByLeaf}
                selectedLeaf={selected?.leafIndex ?? null}
                setSelectedLeaf={setSelectedLeaf}
                claimOne={claimOne}
                claimBatch={claimBatch}
                disabled={!campaignReady || !onTargetChain || transaction.isPending || receipt.isLoading}
              />
            ) : null}
            {campaign.data && tab === "delegate" ? (
              <DelegatePanel entry={selected} payload={signedPayload} onSign={signDelegated} disabled={!campaignReady || !onTargetChain || signer.isPending} />
            ) : null}
            {campaign.data && tab === "relay" ? (
              <RelayPanel value={relayInput} setValue={setRelayInput} onRelay={relay} disabled={!campaignReady || !onTargetChain || !relayInput || transaction.isPending} />
            ) : null}
            {campaign.data && tab === "audit" ? (
              <AuditPanel manifestEntries={campaign.data.manifest.entries.length} claimed={claimedCount.data ?? BigInt(campaign.data.status.claimsCompleted)} status={campaign.data.status.reconciliationStatus} commit={campaign.data.status.lastVerifiedCommit} />
            ) : null}
            <div className="tx-status" role="status" aria-live="polite"><span className={`status-light ${receipt.isSuccess ? "success" : ""}`} />{message || lifecycle}</div>
          </div>

          <aside className="audit-rail">
            <p className="rail-label">Campaign commitment</p>
            <Metric label="Snapshot block" value={campaign.data?.manifest.campaign.snapshotBlock.toLocaleString() ?? "—"} />
            <Metric label="Manifest entries" value={campaign.data?.manifest.entries.length.toLocaleString() ?? "—"} />
            <Metric label="Claimed live" value={claimedCount.data?.toString() ?? "—"} />
            <div className="root-block"><span>Merkle root</span><code>{shortHash(campaign.data?.proofs.root)}</code></div>
            <div className="policy-note"><span>Late transfers</span><strong>Not eligible</strong><p>Ownership is final at the published snapshot block.</p></div>
          </aside>
        </section>
      </main>

      <footer><span>MIT · Inspect every assumption</span><a href="https://github.com/galo13eth/evm-migration-lab">Source ↗</a></footer>
    </div>
  );
}

function ClaimPanel({ isConnected, eligible, claimedByLeaf, selectedLeaf, setSelectedLeaf, claimOne, claimBatch, disabled }: {
  isConnected: boolean;
  eligible: ManifestEntry[];
  claimedByLeaf: Map<number, boolean>;
  selectedLeaf: number | null;
  setSelectedLeaf: (leaf: number) => void;
  claimOne: (entry: ManifestEntry) => void;
  claimBatch: () => void;
  disabled: boolean;
}) {
  if (!isConnected) return <PanelState title="Connect to inspect eligibility" detail="Your address is matched locally against the published manifest." />;
  if (!eligible.length) return <PanelState title="No snapshot holdings" detail="This address has no entries in the committed source snapshot." />;
  const selected = eligible.find((entry) => entry.leafIndex === selectedLeaf) ?? eligible[0]!;
  const batchAvailable = eligible.length > 1 && eligible.every((entry) => !claimedByLeaf.get(entry.leafIndex));
  return <div className="panel-content">
    <PanelHeading index="01" title="Eligible holdings" detail={`${eligible.length} committed ${eligible.length === 1 ? "entry" : "entries"} for this source owner.`} />
    <div className="holdings-list">
      {eligible.map((entry) => <button key={entry.leafIndex} className={`holding-row ${selected.leafIndex === entry.leafIndex ? "selected" : ""}`} onClick={() => setSelectedLeaf(entry.leafIndex)}>
        <span className="token-icon">{entry.standard === 1 ? "721" : "1155"}</span>
        <span><strong>Token #{entry.tokenId}</strong><small>Amount {entry.amount} · leaf {entry.leafIndex}</small></span>
        <span className={claimedByLeaf.get(entry.leafIndex) ? "claim-state claimed" : "claim-state"}>{claimedByLeaf.get(entry.leafIndex) ? "Claimed" : "Ready"}</span>
      </button>)}
    </div>
    <div className="button-row">
      <button className="primary-button" disabled={disabled || claimedByLeaf.get(selected.leafIndex)} onClick={() => claimOne(selected)}>Claim selected <span>→</span></button>
      <button className="secondary-button" disabled={disabled || !batchAvailable} onClick={claimBatch}>Claim all</button>
    </div>
  </div>;
}

function DelegatePanel({ entry, payload, onSign, disabled }: { entry?: ManifestEntry; payload: string; onSign: (entry: ManifestEntry) => void; disabled: boolean }) {
  return <div className="panel-content"><PanelHeading index="02" title="Sign, then relay" detail="Authorize one fixed recipient without giving a relayer custody or redirect power." />
    {entry ? <div className="delegation-card"><div><span>Leaf</span><code>{entry.leafIndex}</code></div><div><span>Recipient</span><code>{shortHash(entry.destinationRecipient)}</code></div><div><span>Expires</span><code>+60 minutes</code></div></div> : <p className="empty-copy">Connect an eligible source owner first.</p>}
    <button className="primary-button" disabled={disabled || !entry} onClick={() => entry && onSign(entry)}>Sign EIP-712 authorization <span>↗</span></button>
    {payload ? <><label className="field-label" htmlFor="signed-payload">Signed relay payload</label><textarea id="signed-payload" className="code-field" readOnly value={payload} /></> : null}
  </div>;
}

function RelayPanel({ value, setValue, onRelay, disabled }: { value: string; setValue: (value: string) => void; onRelay: () => void; disabled: boolean }) {
  return <div className="panel-content"><PanelHeading index="03" title="Permissionless relay" detail="Paste a signed authorization. The contract validates nonce, deadline, leaf, recipient, and EOA or ERC-1271 signature." />
    <label className="field-label" htmlFor="relay-payload">Delegated claim payload</label><textarea id="relay-payload" className="code-field relay" placeholder={'{ "claim": …, "signature": "0x…" }'} value={value} onChange={(event) => setValue(event.target.value)} />
    <button className="primary-button" disabled={disabled} onClick={onRelay}>Submit delegated claim <span>→</span></button>
  </div>;
}

function AuditPanel({ manifestEntries, claimed, status, commit }: { manifestEntries: number; claimed: bigint; status: string; commit: string }) {
  const percent = manifestEntries ? Math.min(100, Number((claimed * 10_000n) / BigInt(manifestEntries)) / 100) : 0;
  return <div className="panel-content"><PanelHeading index="04" title="Reconciliation" detail="Static snapshot totals beside live destination claim state." />
    <div className="reconcile-grid"><Metric label="Manifest" value={manifestEntries.toLocaleString()} /><Metric label="Claimed" value={claimed.toString()} /><Metric label="Coverage" value={`${percent.toFixed(1)}%`} /></div>
    <div className="progress-track"><span style={{ width: `${percent}%` }} /></div>
    <dl className="audit-list"><div><dt>Snapshot verification</dt><dd className="good">{status}</dd></div><div><dt>Last verified commit</dt><dd><code>{shortHash(commit)}</code></dd></div><div><dt>Data source</dt><dd>Static artifacts + chain reads</dd></div></dl>
  </div>;
}

function PanelHeading({ index, title, detail }: { index: string; title: string; detail: string }) { return <div className="panel-heading"><span>{index}</span><div><h2>{title}</h2><p>{detail}</p></div></div>; }
function PanelState({ title, detail }: { title: string; detail: string }) { return <div className="panel-state"><span className="loader-mark">M/</span><h2>{title}</h2><p>{detail}</p></div>; }
function Metric({ label, value }: { label: string; value: string }) { return <div className="metric"><span>{label}</span><strong>{value}</strong></div>; }
function shortHash(value?: string) { return value && value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-6)}` : value ?? "—"; }
function errorMessage(error: unknown) { return error instanceof Error ? error.message.split("\n")[0] : "Unexpected operation failure"; }
