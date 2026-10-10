import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const args = process.argv.slice(2);
const command = args.find((arg) => !arg.startsWith("-"));
const needsRust =
  ["build", "dev", "bundle", "info", "android", "ios"].includes(command) &&
  !args.some((arg) => ["--help", "-h", "--version", "-V"].includes(arg));

try {
  if (needsRust) {
    const env = { ...process.env };
    const probeRust = () =>
      spawnSync("rustc", ["-vV"], { env, encoding: "utf8" });
    let rust = probeRust();

    if (process.platform === "darwin") {
      const localCargoHome = join(root, ".build", "cargo-home");
      if (!env.CARGO_HOME && existsSync(join(localCargoHome, "registry"))) {
        env.CARGO_HOME = localCargoHome;
      }

      if (rust.status !== 0) {
        const brewPath = (flag) => {
          const result = spawnSync("brew", [flag, "z3"], {
            encoding: "utf8",
          });
          return result.status === 0 ? result.stdout.trim() : "";
        };
        const prefix = brewPath("--prefix");
        const cellar = brewPath("--cellar");
        const candidates = prefix ? [join(prefix, "lib")] : [];
        if (cellar && existsSync(cellar)) {
          for (const entry of readdirSync(cellar, { withFileTypes: true })) {
            if (entry.isDirectory()) {
              candidates.push(join(cellar, entry.name, "lib"));
            }
          }
        }
        const originalLibraryPath = env.DYLD_FALLBACK_LIBRARY_PATH;
        for (const libraryPath of new Set(candidates)) {
          if (!existsSync(libraryPath)) continue;
          env.DYLD_FALLBACK_LIBRARY_PATH = [libraryPath, originalLibraryPath]
            .filter(Boolean)
            .join(":");
          rust = probeRust();
          if (rust.status === 0) {
            console.info(`Using ${libraryPath} for the Rust toolchain.`);
            break;
          }
        }
      }

      if (rust.stdout?.includes("(Homebrew)")) {
        // Homebrew strip removes proc-macro metadata needed by this toolchain.
        env.CARGO_PROFILE_RELEASE_STRIP ??= "none";
      }
    }

    if (rust.status !== 0 || !/^host: /m.test(rust.stdout ?? "")) {
      throw new Error(
        `Rust toolchain could not start.\n${rust.error?.message || rust.stderr?.trim() || "rustc -vV did not report a host target."}`,
      );
    }

    // Apply this after pnpm's shell has started: macOS clears DYLD_* when a
    // protected shell starts. Load the CLI directly, without another shell shim.
    Object.assign(process.env, env);
  }

  const require = createRequire(import.meta.url);
  const cli = require.resolve("@tauri-apps/cli/tauri.js");
  process.argv = [process.execPath, cli, ...args];
  require(cli);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
