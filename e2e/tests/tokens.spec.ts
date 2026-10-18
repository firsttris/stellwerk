import { expect, test } from "@playwright/test";

test("an API token is shown once and can be revoked", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "API-Tokens" }).click();
  await expect(page.getByRole("heading", { name: "API-Tokens" })).toBeVisible();
  const list = page.getByRole("region", { name: "Tokens" });
  await expect(list.getByText("Claude Code")).toBeVisible();

  await page.getByLabel("Name").fill("Backup-Skript");
  await page.getByLabel("Umfang").selectOption("read");
  await expect(page.getByText("Ändert nichts.", { exact: false })).toBeVisible();
  await page.getByRole("button", { name: "Token erstellen" }).click();
  const created = page.getByRole("status").filter({ hasText: "Token „Backup-Skript“ erstellt" });
  await expect(created.getByText(/^stw_/)).toBeVisible();
  await expect(list.getByText("Backup-Skript")).toBeVisible();
  await expect(list.getByText("noch nie benutzt", { exact: false })).toBeVisible();

  page.once("dialog", (dialog) => dialog.accept());
  await list.getByRole("button", { name: "Backup-Skript widerrufen" }).click();
  await expect(list.getByText("Backup-Skript")).toBeHidden();
  await expect(created).toBeHidden();
});
