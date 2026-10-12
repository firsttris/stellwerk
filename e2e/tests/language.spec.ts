import { expect, test } from "@playwright/test";

test.describe("in an English browser", () => {
  test.use({ locale: "en-US" });

  test("the UI is English", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await expect(page.getByRole("table", { name: "Share matrix" })).toBeVisible();
    await expect(
      page.getByRole("row", { name: /Photos/ }).getByText("Syncing · 72 %"),
    ).toBeVisible();
    await expect(page.getByText("workbench changed 18 files in Projects")).toBeVisible();
    await page.getByRole("button", { name: /^Photos on laptop: off/ }).click();
    await expect(page.getByText("1 unsaved change.")).toBeVisible();
  });
});

test("the language can be switched and is remembered", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Übersicht" })).toBeVisible();
  await page.getByLabel("Sprache").selectOption("en");
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  await page.getByLabel("Language").selectOption("de");
  await expect(page.getByRole("heading", { name: "Übersicht" })).toBeVisible();
});
