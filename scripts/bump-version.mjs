#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const VERSION_RE = /^\d+\.\d+\.\d+$/;
const args = process.argv.slice(2);
const noPush = args.includes("--no-push");
const dryRun = args.includes("--dry-run");
const help = args.includes("--help") || args.includes("-h");
const version = args.find((arg) => !arg.startsWith("--"));

function usage() {
  console.log(`Usage: yarn release:bump X.X.X [--no-push] [--dry-run]

Updates the app release version, commits it, tags vX.X.X, and pushes the
branch plus tags. The working tree must be clean before running.`);
}

if (help || !version) {
  usage();
  process.exit(help ? 0 : 1);
}

if (!VERSION_RE.test(version)) {
  console.error(`Invalid version "${version}". Expected X.X.X.`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const label = [command, ...args].join(" ");
  const cwd = options.cwd ? `${options.cwd}: ` : "";

  if (dryRun && options.mutates) {
    console.log(`[dry-run] ${cwd}${label}`);
    return "";
  }

  console.log(`$ ${cwd}${label}`);
  return execFileSync(command, args, {
    encoding: "utf8",
    cwd: options.cwd,
    stdio: options.capture ? "pipe" : "inherit",
  });
}

function capture(command, args) {
  return execFileSync(command, args, { encoding: "utf8", stdio: "pipe" }).trim();
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function replacePackageVersion(file, oldVersion, nextVersion) {
  const source = readFileSync(file, "utf8");
  const next = source.replace(
    new RegExp(`(^version\\s*=\\s*)"${oldVersion.replaceAll(".", "\\.")}"`, "m"),
    `$1"${nextVersion}"`,
  );

  if (next !== source) {
    writeFileSync(file, next);
    return true;
  }

  return false;
}

function cargoManifests() {
  const manifests = ["src-tauri/Cargo.toml"];
  const cratesDir = "src-tauri/crates";

  if (existsSync(cratesDir)) {
    for (const entry of readdirSync(cratesDir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        const manifest = join(cratesDir, entry.name, "Cargo.toml");
        if (existsSync(manifest)) {
          manifests.push(manifest);
        }
      }
    }
  }

  return manifests;
}

if (!dryRun) {
  const status = capture("git", ["status", "--porcelain"]);
  if (status) {
    console.error("Working tree is not clean. Commit or stash existing changes first.");
    process.exit(1);
  }
}

try {
  capture("git", ["rev-parse", "-q", "--verify", `refs/tags/v${version}`]);
  console.error(`Tag v${version} already exists.`);
  process.exit(1);
} catch {
  // Tag does not exist locally.
}

const pkg = readJson("package.json");
const oldVersion = pkg.version;

if (!VERSION_RE.test(oldVersion)) {
  console.error(`Current package.json version "${oldVersion}" is not X.X.X.`);
  process.exit(1);
}

if (oldVersion === version) {
  console.error(`Version is already ${version}.`);
  process.exit(1);
}

const changedFiles = ["package.json"];
pkg.version = version;

if (dryRun) {
  console.log(`[dry-run] package.json ${oldVersion} -> ${version}`);
} else {
  writeJson("package.json", pkg);
}

for (const manifest of cargoManifests()) {
  const changed = dryRun
    ? readFileSync(manifest, "utf8").includes(`version = "${oldVersion}"`)
    : replacePackageVersion(manifest, oldVersion, version);

  if (changed) {
    changedFiles.push(manifest);
    console.log(`${manifest} ${oldVersion} -> ${version}`);
  }
}

if (!dryRun) {
  run("cargo", ["check", "--workspace"], { mutates: true, cwd: "src-tauri" });
  changedFiles.push("src-tauri/Cargo.lock");
}

run("git", ["add", ...changedFiles], { mutates: true });
run("git", ["commit", "-m", `chore: release v${version}`], { mutates: true });
run("git", ["tag", `v${version}`], { mutates: true });

if (noPush) {
  console.log(`Created local release commit and tag v${version}. Push manually when ready.`);
} else {
  run("git", ["push"], { mutates: true });
  run("git", ["push", "--tags"], { mutates: true });
}
