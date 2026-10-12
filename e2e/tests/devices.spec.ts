import { expect, test } from "@playwright/test";

test("pairing shows a command, waits for the device and sets it up", async ({ page }) => {
  await page.goto("/devices");
  await page.getByRole("button", { name: "Gerät koppeln" }).click();
  const dialog = page.getByRole("dialog", { name: "Gerät koppeln" });
  await expect(dialog.getByTestId("pair-command")).toContainText(
    "stellwerk-agent join --server http://127.0.0.1:4173 K7Q2-MX9P-41TZ && stellwerk-agent run",
  );
  await dialog.getByRole("tab", { name: "Windows" }).click();
  await expect(dialog.getByTestId("pair-command")).toContainText("stellwerk-agent.exe join");

  await expect(dialog.getByText("Neues Gerät verbunden")).toBeVisible({ timeout: 10_000 });
  await dialog.getByLabel("Name").fill("werkstatt");
  await dialog.getByRole("checkbox", { name: "Dokumente" }).check();
  await dialog.getByRole("button", { name: "Gerät mit 1 Ordner einrichten" }).click();
  await expect(dialog).toBeHidden();

  await expect(page.getByRole("cell", { name: "werkstatt Linux" })).toBeVisible();
  await page.getByRole("link", { name: "Übersicht" }).click();
  await expect(
    page.getByRole("button", { name: /^Dokumente auf werkstatt: senden und empfangen/ }),
  ).toBeVisible();
});

test("search and filter narrow the list", async ({ page }) => {
  await page.goto("/devices");
  await page.getByRole("button", { name: "Offline 1" }).click();
  await expect(page.getByRole("cell", { name: "büro-pc Windows" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "laptop Windows" })).toBeHidden();
  await page.getByRole("button", { name: "Alle 4" }).click();
  await page.getByPlaceholder("Geräte durchsuchen").fill("mac");
  await expect(page.getByRole("cell", { name: "macbook macOS" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "laptop Windows" })).toBeHidden();
});

test("a blocked device says so", async ({ page }) => {
  await page.goto("/devices");
  await page.getByRole("button", { name: "Aktionen für laptop" }).click();
  await page.getByRole("menuitem", { name: "Sperren" }).click();
  await expect(page.getByRole("row", { name: /laptop/ }).getByText("Gesperrt")).toBeVisible();
});
