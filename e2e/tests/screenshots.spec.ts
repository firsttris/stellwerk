import { expect, test } from "@playwright/test";

// Pictures for the README and the docs, from the demo data: `pnpm screenshots`. In English,
// for readers on GitHub; the language test covers the English UI too.

test.use({ viewport: { width: 1440, height: 960 }, deviceScaleFactor: 1, locale: "en-US" });

const shot = (name: string) => `../docs/${name}.png`;

test("overview @screenshots", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("table", { name: "Share matrix" })).toBeVisible();
  await page.getByRole("button", { name: /Photos on laptop/ }).click();
  await page.getByRole("button", { name: /Music on office-pc/ }).click();
  await page.screenshot({ path: shot("screenshot"), fullPage: true });
});

test("devices and pairing @screenshots", async ({ page }) => {
  await page.goto("/devices");
  await expect(page.getByRole("cell", { name: "office-pc Windows" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-devices"), fullPage: true });
  await page.getByRole("button", { name: "Pair device" }).click();
  await expect(page.getByTestId("pair-command")).toContainText("K7Q2-MX9P-41TZ");
  await page.screenshot({ path: shot("screenshot-pairing") });
});

test("folder @screenshots", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Projects" }).click();
  await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-folder"), fullPage: true });
});

test("conflict @screenshots", async ({ page }) => {
  await page.goto("/conflicts");
  await expect(page.getByText("Version from workbench")).toBeVisible();
  await page.screenshot({ path: shot("screenshot-conflict"), fullPage: true });
});

test("dark mode @screenshots", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await expect(page.getByRole("table", { name: "Share matrix" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-dark"), fullPage: true });
});
