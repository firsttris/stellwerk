import { expect, test } from "@playwright/test";

const background = (page: import("@playwright/test").Page) =>
  page.locator("body").evaluate((body) => getComputedStyle(body).backgroundColor);

test("the theme follows the system until one is picked, and is remembered", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  const theme = page.getByRole("group", { name: "Darstellung" });
  await expect(theme.getByRole("radio", { name: "System" })).toBeChecked();
  expect(await background(page)).toBe("rgb(9, 9, 11)");

  await theme.getByText("Hell").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(await background(page)).toBe("rgb(255, 255, 255)");

  await page.reload();
  await expect(theme.getByRole("radio", { name: "Hell" })).toBeChecked();
  expect(await background(page)).toBe("rgb(255, 255, 255)");

  await theme.getByText("Dunkel").click();
  await page.emulateMedia({ colorScheme: "light" });
  expect(await background(page)).toBe("rgb(9, 9, 11)");

  await theme.getByText("System").click();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme");
  expect(await background(page)).toBe("rgb(255, 255, 255)");
});

test("the sidebar shows the installed version", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Version 0.1.8")).toBeVisible();
});
