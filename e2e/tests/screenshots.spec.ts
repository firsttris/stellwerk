import { expect, test } from "@playwright/test";

// Pictures for the README and the docs, from the demo data: `pnpm screenshots`.

test.use({ viewport: { width: 1440, height: 960 }, deviceScaleFactor: 1 });

const shot = (name: string) => `../docs/${name}.png`;

test("overview @screenshots", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("table", { name: "Freigabe-Matrix" })).toBeVisible();
  await page.getByRole("button", { name: /Fotos auf laptop/ }).click();
  await page.getByRole("button", { name: /Musik auf büro-pc/ }).click();
  await page.screenshot({ path: shot("screenshot"), fullPage: true });
});

test("devices and pairing @screenshots", async ({ page }) => {
  await page.goto("/devices");
  await expect(page.getByRole("cell", { name: "büro-pc Windows" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-devices"), fullPage: true });
  await page.getByRole("button", { name: "Gerät koppeln" }).click();
  await expect(page.getByTestId("pair-command")).toContainText("K7Q2-MX9P-41TZ");
  await page.screenshot({ path: shot("screenshot-pairing") });
});

test("folder @screenshots", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "Projekte" }).click();
  await expect(page.getByRole("heading", { name: "Projekte" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-folder"), fullPage: true });
});

test("conflict @screenshots", async ({ page }) => {
  await page.goto("/conflicts");
  await expect(page.getByText("Fassung von werkbank")).toBeVisible();
  await page.screenshot({ path: shot("screenshot-conflict"), fullPage: true });
});

test("dark mode @screenshots", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await expect(page.getByRole("table", { name: "Freigabe-Matrix" })).toBeVisible();
  await page.screenshot({ path: shot("screenshot-dark"), fullPage: true });
});
