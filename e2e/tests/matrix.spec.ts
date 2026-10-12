import { expect, test } from "@playwright/test";

test("a draft collects changes until it is applied or discarded", async ({ page }) => {
  await page.goto("/");
  const cell = page.getByRole("button", { name: /^Fotos auf laptop: aus/ });
  await cell.click();
  await expect(
    page.getByRole("button", { name: /^Fotos auf laptop: senden und empfangen/ }),
  ).toBeVisible();
  await expect(page.getByText("1 ungespeicherte Änderung.")).toBeVisible();
  await expect(
    page.getByText("Wird sofort auf allen betroffenen Geräten angewendet."),
  ).toBeVisible();

  await page.getByRole("button", { name: "Verwerfen" }).click();
  await expect(page.getByText("ungespeicherte Änderung")).toBeHidden();
  await expect(page.getByRole("button", { name: /^Fotos auf laptop: aus/ })).toBeVisible();

  await page.getByRole("button", { name: /^Fotos auf laptop: aus/ }).click();
  await page.getByRole("button", { name: "Anwenden" }).click();
  await expect(
    page.getByText("Angewendet. Alle Geräte haben die neue Konfiguration."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: /^Fotos auf laptop: senden und empfangen/ }),
  ).toBeVisible();
  await expect(page.getByText("Revision 13", { exact: true })).toBeVisible();
});

test("changes for an offline device wait for it", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /^Musik auf büro-pc: aus/ }).click();
  await expect(
    page.getByText("1 davon betrifft ein Offline-Gerät und wird nachgeholt."),
  ).toBeVisible();
  await page.getByRole("button", { name: "Anwenden" }).click();
  await expect(page.getByText(/1 wartet auf Offline-Geräte/)).toBeVisible();
  const musik = page.getByRole("row", { name: /Musik/ });
  await expect(musik.getByText("wartet")).toBeVisible();
});

test("the hub column cannot be switched off", async ({ page }) => {
  await page.goto("/");
  const projekte = page.getByRole("row", { name: /Projekte/ });
  await expect(projekte.getByText("Speichert")).toBeVisible();
  await expect(projekte.getByRole("button", { name: /auf Server/ })).toHaveCount(0);
});

test("a new folder appears as a row", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Neuer Ordner" }).click();
  await page.getByLabel("Name").fill("Spielstände");
  await page.getByRole("button", { name: "Anlegen" }).click();
  await expect(page.getByRole("link", { name: "Spielstände" })).toBeVisible();
});

test("folders and devices show what is happening right now", async ({ page }) => {
  await page.goto("/");
  const fotos = page.getByRole("row", { name: /Fotos/ });
  await expect(fotos.getByText("Läuft · 72 %")).toBeVisible();
  await expect(fotos.getByText("61.004 Dateien · 412 GB")).toBeVisible();
  await expect(page.getByRole("row", { name: /Dokumente/ }).getByText("Fehler")).toBeVisible();
  await expect(page.getByText("Fehler in Dokumente auf laptop")).toBeVisible();
  await expect(page.getByText(/214 Änderungen liegen auf dem Server bereit/)).toBeVisible();
  await expect(page.getByText("werkbank hat 18 Dateien in Projekte geändert")).toBeVisible();

  await page.getByRole("link", { name: "Geräte" }).click();
  await expect(page.getByText("Lädt 18 Änderungen · 72 %")).toBeVisible();
  await expect(page.getByText("214 Änderungen warten auf dem Server")).toBeVisible();
});
