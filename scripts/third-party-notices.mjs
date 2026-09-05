import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const check = process.argv.includes("--check");
const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--locked", "--offline", "--format-version", "1"], {
  cwd: root, encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
}));
const lock = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));
const texts = {};
const packages = [];

for (const pkg of metadata.packages) {
  if (metadata.workspace_members.includes(pkg.id)) continue;
  const patched = pkg.name === "glib" && pkg.source === null;
  packages.push({
    ecosystem: "cargo", name: pkg.name, version: pkg.version,
    license: pkg.license ?? "See included license file",
    source: patched ? "vendor/glib (see vendor/README.md)" : pkg.source,
    homepage: pkg.repository ?? pkg.homepage ?? `https://crates.io/crates/${pkg.name}/${pkg.version}`,
    notices: collectTexts(dirname(pkg.manifest_path), pkg.license_file),
  });
}
for (const [path, entry] of Object.entries(lock.packages)) {
  // Include the production dependency union, not platform-specific development tools.
  if (!path || entry.dev) continue;
  const location = join(root, path);
  if (!existsSync(join(location, "package.json"))) {
    throw new Error(`Run npm ci before collecting notices: ${path}`);
  }
  const manifest = JSON.parse(readFileSync(join(location, "package.json"), "utf8"));
  packages.push({
    ecosystem: "npm", name: manifest.name, version: manifest.version,
    license: manifest.license ?? entry.license ?? "Not declared",
    source: entry.resolved ?? `https://www.npmjs.com/package/${manifest.name}`,
    homepage: manifest.homepage ?? `https://www.npmjs.com/package/${manifest.name}`,
    notices: collectTexts(location),
  });
}
packages.sort((a, b) => compare(`${a.ecosystem}:${a.name}:${a.version}`, `${b.ecosystem}:${b.name}:${b.version}`));
const inventory = {
  schemaVersion: 1,
  scope: "Resolved Rust dependency union (including build/test packages) and production npm dependencies. Not all listed packages are linked into every platform binary. OS-provided frameworks are not included.",
  packages,
  texts: Object.fromEntries(Object.entries(texts).sort(([a], [b]) => compare(a, b))),
};
output("src-tauri/resources/third-party-notices.json", `${JSON.stringify(inventory, null, 2)}\n`);
const missing = packages.filter(pkg => pkg.notices.length === 0);
output("THIRD_PARTY_NOTICES.md", `# Third-Party Notices

Generated from Cargo.lock and package-lock.json. Regenerate with
\`npm run notices:generate\`; CI verifies reproducibility with \`npm run test:notices\`.

The desktop distribution includes \`third-party-notices.json\` in its resources:
package metadata and deduplicated, verbatim license/copyright/notice files.
Rust build/test and all resolved platform dependencies are included conservatively;
the list does not imply every package is linked on every platform. OS frameworks
and user-installed plugins are outside this inventory. External plugin authors
must provide their own licenses. This inventory does not assign a license to Millo.

The GTK-compatible glib patch is documented in [vendor/README.md](vendor/README.md).
Dependencies without a license-text file in their packaged source are listed below;
declared SPDX metadata alone is not a substitute for distribution legal review.

## Packages Without Packaged Notice Text

${missing.map(pkg => `- ${pkg.ecosystem}: ${pkg.name} ${pkg.version} (${pkg.license})`).join("\n") || "None."}

## Dependency Inventory

| Ecosystem | Package | Version | Declared License | Notice Files |
| --- | --- | --- | --- | --- |
${packages.map(pkg => `| ${pkg.ecosystem} | ${pkg.name} | ${pkg.version} | ${String(pkg.license).replaceAll("|", " / ")} | ${pkg.notices.length} |`).join("\n")}
`);
console.log(`${check ? "Verified" : "Generated"} notices for ${packages.length} dependencies (${Object.keys(texts).length} unique texts; ${missing.length} without packaged text)`);

function collectTexts(directory, declared) {
  const candidates = new Set();
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isFile() && /^(licen[cs]e|copying|copyright|notice)([._-]|$)/i.test(entry.name)) candidates.add(entry.name);
    if (entry.isDirectory() && /^(licenses|license|copyrights|notices)$/i.test(entry.name)) {
      for (const child of readdirSync(join(directory, entry.name), { withFileTypes: true })) {
        if (child.isFile()) candidates.add(join(entry.name, child.name));
      }
    }
  }
  if (declared && existsSync(resolve(directory, declared))) candidates.add(relative(directory, resolve(directory, declared)));
  return [...candidates].sort(compare).map(path => {
    const target = realpathSync(join(directory, path));
    if (!target.startsWith(`${realpathSync(directory)}${sep}`)) {
      throw new Error(`Notice path must stay inside its package: ${path}`);
    }
    const text = readFileSync(target, "utf8").replaceAll("\r\n", "\n");
    const sha256 = createHash("sha256").update(text).digest("hex");
    texts[sha256] = text;
    return { path: path.replaceAll("\\", "/"), sha256 };
  });
}
function compare(a, b) { return a < b ? -1 : a > b ? 1 : 0; }
function output(path, content) {
  const target = join(root, path);
  if (check) {
    if (!existsSync(target) || readFileSync(target, "utf8") !== content) {
      throw new Error(`${path} is stale; run npm run notices:generate after fetching locked dependencies`);
    }
  } else {
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
}
