import { defineConfig, devices } from "@playwright/test";

// Against the UI in demo mode (API from fixtures): no Rust, no Syncthing. The CI job runs in
// the official Playwright image; elsewhere PW_CHROMIUM can point at an installed Chromium and
// PW_PORT can move the preview server off 4173 when that port is taken.
const port = Number(process.env.PW_PORT ?? 4173);

export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    locale: "de-DE",
    timezoneId: "Europe/Berlin",
    colorScheme: "light",
    launchOptions: process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `pnpm --filter web build:demo && pnpm --filter web preview:demo`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    stdout: "pipe",
  },
});
