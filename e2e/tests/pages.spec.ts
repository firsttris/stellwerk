import { expect, test } from "@playwright/test";

test("an unknown address says so and leads back", async ({ page }) => {
  await page.goto("/nirgendwo");
  await expect(page.getByRole("heading", { name: "Seite nicht gefunden" })).toBeVisible();
  await page.getByRole("link", { name: "Zur Übersicht" }).click();
  await expect(page.getByRole("heading", { name: "Übersicht" })).toBeVisible();
});
