import { expect, test } from "@playwright/test";

test("mode and path are set per device", async ({ page }) => {
  await page.goto("/folders");
  await page.getByRole("link", { name: /Photos/ }).click();
  await expect(page.getByRole("heading", { name: "Photos" })).toBeVisible();

  const save = page.getByRole("button", { name: "Speichern" });
  await expect(save).toBeDisabled();
  await page.getByLabel("Modus für laptop").selectOption("receiveOnly");
  await page.getByLabel("Pfad auf laptop").fill("D:\\Pictures");
  await save.click();
  // Disabled also while pending; the stored path after leaving and coming back proves the save.
  await page.getByRole("link", { name: "Ordner", exact: true }).click();
  await page.getByRole("link", { name: /Photos/ }).click();
  await expect(page.getByLabel("Pfad auf laptop")).toHaveValue("D:\\Pictures");

  await page.getByRole("link", { name: "Übersicht" }).click();
  await expect(
    page.getByRole("button", { name: /^Photos auf laptop: nur empfangen/ }),
  ).toBeVisible();
});

test("a folder can be renamed and removed", async ({ page }) => {
  page.on("dialog", (dialog) => dialog.accept());
  await page.goto("/folders");
  await page.getByRole("link", { name: /Music/ }).click();
  await page.getByRole("tab", { name: "Einstellungen" }).click();
  await page.getByLabel("Name des Ordners").fill("Hörbücher");
  await page.getByRole("button", { name: "Umbenennen" }).click();
  await expect(page.getByRole("heading", { name: "Hörbücher" })).toBeVisible();

  await page.getByRole("button", { name: "Entfernen …" }).click();
  await expect(page).toHaveURL(/\/folders$/);
  await expect(page.getByRole("link", { name: /Hörbücher/ })).toHaveCount(0);
});

test("ignore patterns are kept per folder", async ({ page }) => {
  await page.goto("/folders");
  await page.getByRole("link", { name: /Projects/ }).click();
  await page.getByRole("tab", { name: "Einstellungen" }).click();
  const patterns = page.getByLabel("Ein Muster pro Zeile");
  await expect(patterns).toHaveValue(/node_modules\//);
  const save = page.getByRole("button", { name: "Muster speichern" });
  await expect(save).toBeDisabled();
  await patterns.fill("node_modules/\n.cache/");
  await save.click();
  await expect(save).toBeDisabled();
  // The demo API lives in the page, so navigate inside the app instead of reloading.
  await page.getByRole("link", { name: "Ordner", exact: true }).click();
  await page.getByRole("link", { name: /Projects/ }).click();
  await page.getByRole("tab", { name: "Einstellungen" }).click();
  await expect(page.getByLabel("Ein Muster pro Zeile")).toHaveValue("node_modules/\n.cache/");
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

  await page.getByPlaceholder("Datei suchen").fill("gibt-es-nicht");
  await expect(page.getByText("Keine Versionen zu dieser Suche.")).toBeVisible();
});

test("a folder that no longer exists says so", async ({ page }) => {
  await page.goto("/folders/gibt-es-nicht");
  await expect(page.getByText("Diesen Ordner gibt es nicht mehr.")).toBeVisible();
  await page.getByRole("link", { name: "Zu den Ordnern" }).click();
  await expect(page.getByRole("heading", { name: "Ordner" })).toBeVisible();
});
