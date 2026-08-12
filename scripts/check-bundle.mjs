import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const assets = new URL("../dist/assets", import.meta.url).pathname;
const javascript = readdirSync(assets).filter((file) => file.endsWith(".js"));
const limits = {
  initial: 500 * 1024,
  lazy: 600 * 1024,
};

if (javascript.length === 0) {
  throw new Error("production build contains no JavaScript assets");
}

for (const file of javascript) {
  const bytes = statSync(join(assets, file)).size;
  const limit = file.startsWith("index-") ? limits.initial : limits.lazy;
  if (bytes > limit) {
    throw new Error(
      `${file} is ${format(bytes)}, exceeding its ${format(limit)} bundle budget`,
    );
  }
}

console.log(
  `Bundle budgets are intact: initial <= ${format(limits.initial)}, lazy <= ${format(limits.lazy)}`,
);

function format(bytes) {
  return `${(bytes / 1024).toFixed(1)} KiB`;
}
