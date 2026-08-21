// @vitest-environment jsdom

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { IntegrityChecks } from "./App";
import { parseCampaignArtifacts, parseRelayPayload } from "./campaign";

const artifact = (name: string) =>
  readFileSync(resolve(process.cwd(), "public", "campaign", name), "utf8");
const valid = () => ({
  manifest: artifact("manifest.json"),
  proofs: artifact("proofs.json"),
  status: artifact("status.json"),
  digests: artifact("artifact-digests.json"),
});

describe("campaign trust boundary", () => {
  it("accepts the committed artifact bundle", () => {
    const campaign = parseCampaignArtifacts(valid());
    expect(campaign.manifest.format).toBe("evm-migration-manifest-v2");
    expect(campaign.status.reconciliationStatus).toBe("sample-consistent");
  });

  it("rejects numeric JSON values instead of risking unsafe JavaScript numbers", () => {
    const raw = valid();
    const manifest = JSON.parse(raw.manifest) as { entries: { leafIndex: unknown }[] };
    manifest.entries[0]!.leafIndex = 0;
    raw.manifest = JSON.stringify(manifest);
    expect(() => parseCampaignArtifacts(raw)).toThrow("must be a decimal string");
  });

  it("rejects proof elements that are not exactly 32 bytes", () => {
    const raw = valid();
    const proofs = JSON.parse(raw.proofs) as { singleProofs: { proof: string[] }[] };
    proofs.singleProofs[0]!.proof[0] = "0x12";
    raw.proofs = JSON.stringify(proofs);
    expect(() => parseCampaignArtifacts(raw)).toThrow("must be 32 bytes");
  });

  it("rejects a status root that differs from the proof root", () => {
    const raw = valid();
    const status = JSON.parse(raw.status) as { merkleRoot: string };
    status.merkleRoot = `0x${"11".repeat(32)}`;
    raw.status = JSON.stringify(status);
    expect(() => parseCampaignArtifacts(raw)).toThrow("Status and proof roots differ");
  });

  it("rejects an artifact digest index that does not commit to itself", () => {
    const raw = valid();
    const digests = JSON.parse(raw.digests) as { bundleDigest: string };
    digests.bundleDigest = `0x${"22".repeat(32)}`;
    raw.digests = JSON.stringify(digests);
    expect(() => parseCampaignArtifacts(raw)).toThrow("Artifact bundle digest is invalid");
  });

  it("rejects malformed relay payloads", () => {
    expect(() => parseRelayPayload('{"proof":["0x12"]}')).toThrow();
  });
});

it("renders failed integrity checks visibly", () => {
  render(<IntegrityChecks checks={[{ label: "Manifest root", ok: false, detail: "mismatch" }]} />);
  expect(screen.getByText("Manifest root").closest("li")).toHaveClass("failed");
  expect(screen.getByText("mismatch")).toBeVisible();
});
