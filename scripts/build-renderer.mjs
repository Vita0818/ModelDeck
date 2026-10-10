import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const steps = [
  ["node_modules/vite/bin/vite.js", "build", ...process.argv.slice(2)],
  ["scripts/verify-renderer.mjs"],
];

for (const [script, ...args] of steps) {
  const result = spawnSync(process.execPath, [join(root, script), ...args], {
    cwd: root,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    if (result.error) console.error(result.error.message);
    process.exit(result.status ?? 1);
  }
}
