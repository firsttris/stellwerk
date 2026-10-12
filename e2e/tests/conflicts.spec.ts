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

test("versions are listed per file and can be restored", async ({ page }) => {
  page.on("dialog", (dialog) => dialog.accept());
  await page.goto("/folders");
  await page.getByRole("link", { name: /Projects/ }).click();
  await page.getByRole("tab", { name: "Versionen" }).click();
  await expect(page.getByText("haul/src/config.ts")).toBeVisible();
  await expect(page.getByText("README.md")).toBeVisible();

  await page.getByPlaceholder("Datei suchen").fill("readme");
  await expect(page.getByText("haul/src/config.ts")).toBeHidden();
  await page.getByRole("button", { name: /README.md vom .* wiederherstellen/ }).click();
  await expect(page.getByText("README.md wiederhergestellt.")).toBeVisible();
});
