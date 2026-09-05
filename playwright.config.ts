import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/workflow",
  globalSetup: "./tests/workflow/browserSetup.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://127.0.0.1:1433",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    browserName: "chromium",
  },
  projects: [
    { name: "desktop", use: { viewport: { width: 1440, height: 960 } } },
    { name: "compact", use: { viewport: { width: 860, height: 600 } } },
    { name: "mobile", use: { viewport: { width: 390, height: 844 } } },
    { name: "webkit-desktop", use: { browserName: "webkit", viewport: { width: 1440, height: 960 } } },
    { name: "webkit-mobile", use: { browserName: "webkit", viewport: { width: 390, height: 844 } } },
  ],
  webServer: {
    command: "npm run dev -- --port 1433",
    url: "http://127.0.0.1:1433",
    reuseExistingServer: !process.env.CI,
  },
});
