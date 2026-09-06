import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".", testMatch: "editor-large.spec.ts", workers: 1,
  outputDir: "../../tmp/editor-large-results",
  timeout: 60_000,
  use: { baseURL: "http://127.0.0.1:1439", screenshot: "only-on-failure" },
  projects: [
    { name: "desktop", use: { viewport: { width: 1440, height: 960 } } },
    { name: "mobile", use: { viewport: { width: 390, height: 844 } } },
    { name: "narrow", use: { viewport: { width: 320, height: 740 } } },
  ],
  webServer: { command: "npm run dev -- --port 1439", url: "http://127.0.0.1:1439", reuseExistingServer: true },
});
