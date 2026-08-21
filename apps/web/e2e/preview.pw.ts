import { expect, test } from "@playwright/test";

test("labels the local artifact preview and keeps audit data accessible", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("LOCAL ARTIFACT DEMO")).toBeVisible();
  await expect(page.getByRole("status")).toContainText("Preview only");
  await page.getByRole("button", { name: "04 Reconcile" }).click();
  await expect(page.getByRole("heading", { name: "CAMPAIGN INTEGRITY" })).toBeVisible();
  await expect(page.getByText("sample-consistent", { exact: true })).toBeVisible();
});

test("fails closed when a bundled artifact does not match", async ({ page }) => {
  await page.route("**/campaign/status.json", async (route) => {
    const response = await route.fetch();
    const status = await response.json();
    status.merkleRoot = `0x${"11".repeat(32)}`;
    await route.fulfill({ response, json: status });
  });

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Artifacts unavailable" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Claim selected/ })).toHaveCount(0);
});
