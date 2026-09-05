import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import ts from "typescript";

const root = new URL("../", import.meta.url);
const sourceRoot = new URL("../src", import.meta.url);
const violations = [];
const moduleBudgets = {
  "crates/millo-command/src/lib.rs": 550,
  "src-tauri/src/commands.rs": 1150,
  "src/App.tsx": 750,
  "src/app/useWorkstation.ts": 850,
  "src/features/program/ProgramWorkspace.tsx": 750,
  "src/features/program/useProgramWorkspace.ts": 950,
  "src/styles.css": 80,
};
for (const [path, limit] of Object.entries(moduleBudgets)) {
  if (readFileSync(new URL(`../${path}`, import.meta.url), "utf8").split("\n").length > limit) {
    violations.push(`${path}: exceeds ${limit} lines; extract a responsibility before growing the coordinator`);
  }
}
for (const name of readdirSync(new URL("../src/styles", import.meta.url))) {
  if (name.endsWith(".css") && readFileSync(new URL(`../src/styles/${name}`, import.meta.url), "utf8").split("\n").length > 900) {
    violations.push(`src/styles/${name}: feature stylesheet exceeds 900 lines`);
  }
}

for (const path of sourceFiles(sourceRoot.pathname)) {
  const projectPath = relative(root.pathname, path);
  const source = readFileSync(path, "utf8");
  const imports = moduleSpecifiers(source, path);

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

function moduleSpecifiers(source, path) {
  const matches = [];
  const tree = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
        node.moduleSpecifier && ts.isStringLiteralLike(node.moduleSpecifier)) {
      matches.push(node.moduleSpecifier.text);
    }
    if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword &&
        node.arguments[0] && ts.isStringLiteralLike(node.arguments[0])) {
      matches.push(node.arguments[0].text);
    }
    if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument) && ts.isStringLiteral(node.argument.literal)) {
      matches.push(node.argument.literal.text);
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
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
