import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
const tauriConfig = JSON.parse(
  await readFile(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
);
const workspace = await readFile(path.join(root, "Cargo.toml"), "utf8");
const app = await readFile(path.join(root, "src/App.tsx"), "utf8");
const html = await readFile(path.join(root, "index.html"), "utf8");

assert.equal(packageJson.name, "millo");
assert.equal(tauriConfig.productName, "Millo");
assert.equal(tauriConfig.identifier, "io.millo.desktop");
assert.match(workspace, /crates\/millo-controller/);
assert.match(app, />Millo</);
assert.match(html, /<title>Millo<\/title>/);

const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".md",
  ".mjs",
  ".rs",
  ".svg",
  ".toml",
  ".ts",
  ".tsx",
]);
const ignoredDirectories = new Set([
  ".git",
  "dist",
  "gen",
  "icons",
  "node_modules",
  "target",
]);
const legacyBrand = ["gantry", "on"].join("");
const staleFiles = [];

async function scan(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.isDirectory() && ignoredDirectories.has(entry.name)) continue;

    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await scan(absolutePath);
      continue;
    }
    if (!textExtensions.has(path.extname(entry.name))) continue;

    const contents = await readFile(absolutePath, "utf8");
    if (contents.toLowerCase().includes(legacyBrand)) {
      staleFiles.push(path.relative(root, absolutePath));
    }
  }
}

await scan(root);
assert.deepEqual(staleFiles, [], `legacy brand remains in: ${staleFiles.join(", ")}`);

console.log("brand contract ok: Millo");
