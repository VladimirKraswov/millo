import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const packageJson = JSON.parse(
  await readFile(new URL("../package.json", import.meta.url), "utf8"),
);
const tauriConfig = JSON.parse(
  await readFile(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
const workspace = await readFile(new URL("../Cargo.toml", import.meta.url), "utf8");

assert.equal(tauriConfig.version, packageJson.version, "Tauri and npm versions differ");
assert.match(
  workspace,
  new RegExp(`\\[workspace\\.package\\][\\s\\S]*?version = "${packageJson.version.replaceAll(".", "\\.")}"`),
  "Cargo workspace version differs from the desktop version",
);
assert.equal(packageJson.milloRelease?.channel, "alpha");
assert.ok(Number.isSafeInteger(packageJson.milloRelease?.sequence));
assert.ok(packageJson.milloRelease.sequence > 0);
assert.match(packageJson.scripts?.["bundle:mac:alpha"] ?? "", /APPLE_SIGNING_IDENTITY=-/);
assert.match(packageJson.scripts?.["bundle:mac:alpha"] ?? "", /--bundles dmg/);
assert.match(packageJson.scripts?.["bundle:linux"] ?? "", /--bundles deb,appimage/);

const targets = new Set(tauriConfig.bundle?.targets ?? []);
for (const target of ["dmg", "deb", "appimage"]) {
  assert.ok(targets.has(target), `missing release bundle target: ${target}`);
}

console.log(
  `release contract ok: v${packageJson.version}-${packageJson.milloRelease.channel}.${packageJson.milloRelease.sequence}`,
);
