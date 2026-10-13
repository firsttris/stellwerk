import { expect, test } from "@playwright/test";

test("pairing shows a command, waits for the device and sets it up", async ({ page }) => {
  await page.goto("/devices");
  await page.getByRole("button", { name: "Gerät koppeln" }).click();
  const dialog = page.getByRole("dialog", { name: "Gerät koppeln" });
  const origin = new URL(page.url()).origin;
  // The demo Zentrale reports version 0.1.3 (src/demo/api.ts).
  const releases = "https://github.com/firsttris/stellwerk/releases/download/v0.1.3";
  await expect(dialog.getByTestId("pair-command")).toContainText(
    `curl -fsSL ${releases}/install.sh | sh -s -- ${origin} K7Q2-MX9P-41TZ`,
  );
  await dialog.getByRole("tab", { name: "Windows" }).click();
  await expect(dialog.getByTestId("pair-command")).toContainText(
    `& ([scriptblock]::Create((irm ${releases}/install.ps1))) ${origin} K7Q2-MX9P-41TZ`,
  );
  await expect(dialog.getByText("In PowerShell ausführen", { exact: false })).toBeVisible();

  await expect(dialog.getByText("Neues Gerät verbunden")).toBeVisible({ timeout: 10_000 });
  await dialog.getByLabel("Name").fill("workshop");
  await dialog.getByRole("checkbox", { name: "Documents" }).check();
  await dialog.getByRole("button", { name: "Gerät mit 1 Ordner einrichten" }).click();
  await expect(dialog).toBeHidden();

  await expect(page.getByRole("cell", { name: "workshop Linux" })).toBeVisible();
  await page.getByRole("link", { name: "Übersicht" }).click();
  await expect(
    page.getByRole("button", { name: /^Documents auf workshop: senden und empfangen/ }),
  ).toBeVisible();
});

test("search and filter narrow the list", async ({ page }) => {
  await page.goto("/devices");
  await page.getByRole("button", { name: "Offline 1" }).click();
  await expect(page.getByRole("cell", { name: "office-pc Windows" })).toBeVisible();
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

test("a device waiting in the LAN is approved with its fingerprint", async ({ page }) => {
  await page.goto("/");
  const attention = page.getByRole("region", { name: "Braucht Aufmerksamkeit" });
  await expect(attention.getByText("living-room-pc wartet auf Freigabe")).toBeVisible();
  await expect(attention.getByText("9C41-07AB-E2D5-3F18")).toBeVisible();
  await attention.getByRole("link", { name: "Prüfen" }).click();

  const waiting = page.getByRole("region", { name: "Wartende Geräte" });
  await expect(waiting.getByText("9C41-07AB-E2D5-3F18")).toBeVisible();
  await waiting.getByRole("button", { name: "living-room-pc freigeben" }).click();
  await expect(waiting).toBeHidden();
  await expect(page.getByRole("cell", { name: "living-room-pc Windows" })).toBeVisible();
});

test("a device waiting in the LAN can be rejected", async ({ page }) => {
  await page.goto("/devices");
  const waiting = page.getByRole("region", { name: "Wartende Geräte" });
  await waiting.getByRole("button", { name: "living-room-pc ablehnen" }).click();
  await expect(waiting).toBeHidden();
  await expect(page.getByRole("cell", { name: "living-room-pc Windows" })).toHaveCount(0);
});

test("a device can be removed", async ({ page }) => {
  page.on("dialog", (dialog) => dialog.accept());
  await page.goto("/devices");
  await page.getByRole("button", { name: "Aktionen für macbook" }).click();
  await page.getByRole("menuitem", { name: "Entfernen …" }).click();
  await expect(page.getByRole("cell", { name: "macbook macOS" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Alle 3" })).toBeVisible();
});
