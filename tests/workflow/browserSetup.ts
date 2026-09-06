import { chromium, webkit, type FullConfig } from "@playwright/test";
import { execFileSync } from "node:child_process";

/** Fail once at the runtime boundary instead of timing out every application scenario. */
export default async function browserSetup(config: FullConfig) {
  execFileSync("cargo", ["build", "-p", "millo-sketch", "--example", "fixture", "--locked"], {
    timeout: 300_000, stdio: "inherit",
  });
  const engines = new Set(config.projects.map(project => project.use.browserName ?? "chromium"));
  for (const engine of engines) {
    if (engine !== "chromium" && engine !== "webkit") {
      throw new Error(`Add a runtime smoke check for the new browser engine: ${engine}`);
    }
    const browser = await ({ chromium, webkit }[engine]).launch({ timeout: 15_000 });
    try {
      const page = await browser.newPage();
      await page.setContent("<title>Millo browser runtime</title>");
      if (await page.title() !== "Millo browser runtime") throw new Error("Browser did not create a working page");
    } catch (cause) {
      throw new Error(`${engine} runtime is incompatible. Reinstall locked Playwright browsers; current WebKit CI uses macOS 15 or Ubuntu 24.04.`, { cause });
    } finally {
      await browser.close();
    }
  }
}
