import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve, sep } from "node:path";

const root = new URL("../", import.meta.url);
const sourceRoot = new URL("../src", import.meta.url);
const violations = [];

for (const path of sourceFiles(sourceRoot.pathname)) {
  const projectPath = relative(root.pathname, path);
  const source = readFileSync(path, "utf8");
  const imports = moduleSpecifiers(source);

  if (
    imports.some((specifier) => specifier.startsWith("@tauri-apps/")) &&
    !projectPath.startsWith("src/api/") &&
    !projectPath.split("/").at(-1)?.startsWith("tauri")
  ) {
    violations.push(`${projectPath}: Tauri imports belong in API/gateway adapters`);
  }

  if (
    projectPath.startsWith("src/plugins/") &&
    !projectPath.includes(".test.") &&
    imports.some((specifier) =>
      resolvesInside(path, specifier, [
        new URL("../src/platform/plugins", import.meta.url).pathname,
        new URL("../src/platform/extensions", import.meta.url).pathname,
      ]),
    )
  ) {
    violations.push(`${projectPath}: plugins must import their host contract from src/plugin-sdk`);
  }

  if (
    projectPath.startsWith("src/plugins/") &&
    !projectPath.includes(".test.") &&
    imports.some(
      (specifier) =>
        specifier.startsWith("@tauri-apps/") ||
        resolvesInside(path, specifier, [new URL("../src/api", import.meta.url).pathname]),
    )
  ) {
    violations.push(`${projectPath}: plugins cannot bypass typed capabilities through Tauri/API imports`);
  }
}

if (violations.length > 0) {
  throw new Error(`Architecture boundary violations:\n${violations.join("\n")}`);
}

console.log("Frontend architecture boundaries are intact");

function sourceFiles(directory) {
  return readdirSync(directory).flatMap((entry) => {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) return sourceFiles(path);
    return [".ts", ".tsx"].includes(extname(path)) ? [path] : [];
  });
}

function moduleSpecifiers(source) {
  const matches = [];
  for (const pattern of [
    /\bfrom\s*["']([^"']+)["']/g,
    /\bimport\s*["']([^"']+)["']/g,
    /\bimport\s*\(\s*["']([^"']+)["']\s*\)/g,
  ]) {
    for (const match of source.matchAll(pattern)) matches.push(match[1]);
  }
  return matches;
}

function resolvesInside(importer, specifier, forbiddenRoots) {
  const target = resolveSpecifier(importer, specifier);
  return (
    target !== undefined &&
    forbiddenRoots.some((forbidden) => target === forbidden || target.startsWith(`${forbidden}${sep}`))
  );
}

function resolveSpecifier(importer, specifier) {
  if (specifier.startsWith(".")) return resolve(dirname(importer), specifier);
  if (specifier.startsWith("/src/")) return resolve(root.pathname, `.${specifier}`);
  if (specifier.startsWith("src/")) return resolve(root.pathname, specifier);
  return undefined;
}
