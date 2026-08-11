import { readFileSync } from "node:fs";

const config = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);
const security = config.app?.security;
const requiredDirectives = [
  "default-src",
  "connect-src",
  "script-src",
  "style-src",
  "object-src",
  "base-uri",
  "frame-ancestors",
];

for (const [label, policy] of [
  ["csp", security?.csp],
  ["devCsp", security?.devCsp],
]) {
  if (!policy || typeof policy !== "object") {
    throw new Error(`${label} must be an explicit directive map`);
  }
  for (const directive of requiredDirectives) {
    if (typeof policy[directive] !== "string" || policy[directive].trim() === "") {
      throw new Error(`${label} is missing ${directive}`);
    }
  }
  const serialized = JSON.stringify(policy);
  if (serialized.includes("'unsafe-eval'") || serialized.includes("*")) {
    throw new Error(`${label} contains an unsafe wildcard or eval source`);
  }
}

if (security.csp["connect-src"] !== "ipc: http://ipc.localhost") {
  throw new Error("production connect-src must expose only Tauri IPC");
}

console.log("Tauri CSP security contract is configured");
