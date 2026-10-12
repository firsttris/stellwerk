// Renders social-preview.html to docs/social-preview.png (1280 × 640), from the screenshots in
// docs/, so run it after them: `pnpm screenshots` does both.
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
// Playwright is a dependency of e2e/, not of the root package
const { chromium } = createRequire(path.resolve(here, "../../e2e/package.json"))("@playwright/test");
const out = path.resolve(here, "../../docs/social-preview.png");
const browser = await chromium.launch(
  process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
);
const page = await browser.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
await page.goto(`file://${path.join(here, "social-preview.html")}`);
await page.evaluate(() => document.fonts.ready);
await page.screenshot({ path: out, type: "png" });
await browser.close();
console.log("written", out);
