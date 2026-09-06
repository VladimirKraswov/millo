import { defineConfig } from "@playwright/test";

// Feature-only browser tests: no Rust fixtures, hardware, or build steps.
export default defineConfig({
  testDir: ".", testMatch: ["program-lifecycle.spec.ts", "operator.spec.ts"], workers: 1,
  use: { baseURL: "http://127.0.0.1:1435", screenshot: "only-on-failure" },
  projects: [
    { name: "desktop", use: { viewport: { width: 1440, height: 960 } } },
    { name: "mobile", use: { viewport: { width: 390, height: 844 } } },
  ],
  webServer: { command: "npm run dev -- --port 1435", url: "http://127.0.0.1:1435", reuseExistingServer: true },
});
