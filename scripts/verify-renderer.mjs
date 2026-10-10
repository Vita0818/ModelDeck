import { readdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const assets = resolve(root, process.argv[2] ?? "dist", "assets");
const files = readdirSync(assets);
const groups = [
  "react",
  "vendor",
  "editor",
  "radix",
  "i18n",
  "motion",
  "charts",
];
let checked = 0;

// Evaluate the emitted library modules with the native ESM loader. A successful
// Vite build alone does not catch initialization cycles between manual chunks.
try {
  for (const group of groups) {
    const chunks = files.filter(
      (file) => file.startsWith(`${group}-`) && file.endsWith(".js"),
    );
    for (const chunk of chunks) {
      await import(pathToFileURL(join(assets, chunk)).href);
      checked += 1;
    }
  }
  if (checked === 0) {
    throw new Error(
      "No renderer library chunks found. Run the Vite build first.",
    );
  }
  console.info(`Renderer check passed: ${checked} library chunks initialized.`);
} catch (error) {
  console.error("Renderer chunk initialization failed:", error);
  process.exitCode = 1;
}
