import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("../", import.meta.url));
const source = readFileSync(resolve(root, "vendor/glib/src/variant_iter.rs"));
const expected = "a0f5ee8acb8faa089bcdfbc9a57372609fce7654026ccef7d9a224d05a654ccc";
if (createHash("sha256").update(source).digest("hex") !== expected) {
  throw new Error("glib iterator patch changed: review upstream provenance and release regression");
}
const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--offline", "--format-version", "1"], {
  cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
}));
const packages = metadata.packages.filter(pkg => pkg.name === "glib");
if (packages.length !== 1 || packages[0].version !== "0.18.5" || packages[0].source !== null ||
    resolve(packages[0].manifest_path) !== resolve(root, "vendor/glib/Cargo.toml")) {
  throw new Error("The entire GTK graph must resolve the maintained local glib patch, without an unpatched second copy");
}
for (const name of ["LICENSE", "COPYRIGHT"]) {
  if (!existsSync(resolve(root, "vendor/glib", name))) throw new Error(`Missing upstream glib ${name}`);
}
console.log("glib patch fingerprint, provenance notices and dependency graph verified");
