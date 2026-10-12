import { defineConfig, devices } from "@playwright/test";

// Against the UI in demo mode (API from fixtures): no Rust, no Syncthing. The CI job runs in
// the official Playwright image; elsewhere PW_CHROMIUM can point at an installed Chromium.
export default defineConfig({
  testDir: "tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["github"], ["list"]] : "list",
  use: {
    baseURL: "http://127.0.0.1:4173",
    locale: "de-DE",
    timezoneId: "Europe/Berlin",
    colorScheme: "light",
    launchOptions: process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "pnpm --filter web build:demo && pnpm --filter web preview:demo",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    stdout: "pipe",
  },
});
