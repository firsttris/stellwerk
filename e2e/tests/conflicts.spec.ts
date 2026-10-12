import { expect, test } from "@playwright/test";

test("a conflict is shown side by side and resolved", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("row", { name: /Projects/ }).getByText("1 Konflikt")).toBeVisible();
  await expect(page.getByText("Konflikt in Projects")).toBeVisible();
  await page.getByRole("link", { name: "Lösen" }).click();

  await expect(page).toHaveURL(/tab=conflicts/);
  const card = page.getByRole("region", { name: "Konflikt in haul/src/config.ts" });
  await expect(card.getByText("Fassung von workbench")).toBeVisible();
  await expect(card.getByText("+ timeoutMs: 30_000,")).toBeVisible();
  await expect(card.getByText("- retries: 5,")).toBeVisible();
  await expect(card.getByText("config.workbench.ts")).toBeVisible();

  await card.getByRole("button", { name: "Beide behalten" }).click();
  await expect(page.getByText("Keine Konflikte.")).toBeVisible();
  await expect(page.getByRole("link", { name: /^Konflikte/ })).toHaveText("Konflikte");
});

test("keeping one side clears the conflict page", async ({ page }) => {
  await page.goto("/conflicts");
  const card = page.getByRole("region", { name: "Konflikt in haul/src/config.ts" });
  await card.getByRole("button", { name: "Diese Fassung behalten" }).first().click();
  await expect(card).toBeHidden();
  await expect(page.getByText("Keine Konflikte. Alles ist eindeutig.")).toBeVisible();
});
