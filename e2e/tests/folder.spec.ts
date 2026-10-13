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

test("a new folder starts from a folder picked on a device", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Neuer Ordner" }).click();
  await expect(page.getByLabel("Gerät")).toHaveValue("workbench");

  await page.getByRole("button", { name: "Ordner auf workbench auswählen" }).click();
  const picker = page.getByRole("dialog", { name: "Ordner auf workbench" });
  await picker.getByRole("button", { name: "Pictures" }).click();
  await picker.getByRole("button", { name: "Holidays 2026" }).click();
  await expect(picker.getByLabel("Pfad", { exact: true })).toHaveValue(
    "/home/demo/Pictures/Holidays 2026",
  );
  await picker.getByRole("button", { name: "Diesen Ordner wählen" }).click();
  await expect(picker).toBeHidden();

  await expect(page.getByRole("textbox", { name: "Ordner auf workbench" })).toHaveValue(
    "/home/demo/Pictures/Holidays 2026",
  );
  await expect(page.getByLabel("Name")).toHaveValue("Holidays 2026");
  await page.getByRole("button", { name: "Anlegen" }).click();

  await page.getByRole("link", { name: "Holidays 2026" }).click();
  await expect(page.getByLabel("Modus für workbench")).toHaveValue("sendReceive");
  await expect(page.getByLabel("Pfad auf workbench")).toHaveValue(
    "/home/demo/Pictures/Holidays 2026",
  );
});

test("the picker walks a Windows device's drives and is off for offline devices", async ({
  page,
}) => {
  await page.goto("/folders");
  await page.getByRole("link", { name: /Photos/ }).click();
  await expect(
    page.getByRole("button", { name: "office-pc ist offline – Pfad von Hand eintragen" }),
  ).toBeDisabled();
  // Not on the laptop yet: the picker opens once the folder is.
  await expect(page.getByRole("button", { name: "Ordner auf laptop auswählen" })).toBeDisabled();
  await page.getByLabel("Modus für laptop").selectOption("sendReceive");

  await page.getByRole("button", { name: "Ordner auf laptop auswählen" }).click();
  const picker = page.getByRole("dialog", { name: "Ordner auf laptop" });
  await picker.getByRole("button", { name: "Persönlicher Ordner" }).click();
  await expect(picker.getByLabel("Pfad", { exact: true })).toHaveValue("C:\\Users\\demo");
  await picker.getByRole("button", { name: "Eine Ebene höher" }).click();
  await expect(picker.getByLabel("Pfad", { exact: true })).toHaveValue("C:\\Users");
  await picker.getByRole("button", { name: "D:\\" }).click();
  await expect(picker.getByText("Keine Unterordner.")).toBeVisible();
  await picker.getByLabel("Pfad", { exact: true }).fill("C:\\Users\\demo\\Pictures");
  await picker.getByLabel("Pfad", { exact: true }).press("Enter");
  await picker.getByRole("button", { name: "Phone" }).click();
  await picker.getByRole("button", { name: "Diesen Ordner wählen" }).click();

  await expect(page.getByLabel("Pfad auf laptop")).toHaveValue("C:\\Users\\demo\\Pictures\\Phone");
  await page.getByRole("button", { name: "Speichern" }).click();
  await page.getByRole("link", { name: "Ordner", exact: true }).click();
  await page.getByRole("link", { name: /Photos/ }).click();
  await expect(page.getByLabel("Pfad auf laptop")).toHaveValue("C:\\Users\\demo\\Pictures\\Phone");
});
