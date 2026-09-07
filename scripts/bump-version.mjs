#!/usr/bin/env node

// Legacy entry point delegates to the Rust version source. Git publication is explicit.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const help = args.includes("--help") || args.includes("-h");
const versions = args.filter((argument) => !argument.startsWith("-"));
const unknown = args.filter((argument) => argument.startsWith("-") && !["--dry-run", "--no-push", "--help", "-h"].includes(argument));
if (help || versions.length !== 1 || unknown.length) {
  console.log(`Usage: yarn release:bump <semver> [--dry-run]

Delegates to cargo run -p xtask -- set-version. Updates workspace/package
versions and Cargo.lock locally; review and commit/tag/push explicitly.
The legacy --no-push option is accepted and has no additional effect.`);
  process.exit(help ? 0 : 1);
}
try {
  execFileSync("cargo", ["run", "-p", "xtask", "--locked", "--", "set-version", versions[0], ...(args.includes("--dry-run") ? ["--dry-run"] : [])], {
    cwd: fileURLToPath(new URL("../", import.meta.url)),
    stdio: "inherit",
  });
} catch (error) {
  process.exit(error.status ?? 1);
}
