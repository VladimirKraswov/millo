import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".", testMatch: "program-documents.spec.ts", workers: 1,
  outputDir: "../../tmp/program-documents-results", timeout: 60_000,
  use: { baseURL: "http://127.0.0.1:1441", screenshot: "only-on-failure" },
  projects: [
    { name: "desktop", use: { viewport: { width: 1440, height: 960 } } },
    { name: "mobile", use: { viewport: { width: 390, height: 844 } } },
  ],
  webServer: { command: "npm run dev -- --port 1441", url: "http://127.0.0.1:1441", reuseExistingServer: true },
});
