#!/usr/bin/env node
"use strict";

const childProcess = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const vscodeDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(vscodeDir, "..");

function run(command, { cwd } = {}) {
  childProcess.execSync(command, { stdio: "inherit", cwd: cwd ?? vscodeDir });
}

function readJson(jsonPath) {
  return JSON.parse(fs.readFileSync(jsonPath, "utf8"));
}

function writeJson(jsonPath, value) {
  fs.writeFileSync(jsonPath, JSON.stringify(value, null, 2) + "\n");
}

function bumpPatchVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:-.+)?$/.exec(version);
  if (!match) {
    throw new Error(`Unsupported version format: ${version}`);
  }
  const major = Number(match[1]);
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  return `${major}.${minor}.${patch + 1}`;
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function copyFile(src, dst) {
  ensureDir(path.dirname(dst));
  fs.copyFileSync(src, dst);
}

function chmodExecutable(filePath) {
  if (process.platform === "win32") return;
  try {
    fs.chmodSync(filePath, 0o755);
  } catch (err) {
    throw new Error(`Failed to chmod +x ${filePath}: ${err instanceof Error ? err.message : String(err)}`);
  }
}

function syncIcons() {
  const srcDir = path.join(repoRoot, "assets");
  const dstDir = path.join(vscodeDir, "assets");
  ensureDir(dstDir);

  for (const name of fs.readdirSync(srcDir)) {
    if (!name.startsWith("aivi") || !name.endsWith(".png")) continue;
    copyFile(path.join(srcDir, name), path.join(dstDir, name));
  }
}

function buildLsp() {
  run("cargo build -p aivi-lsp --release", { cwd: repoRoot });

  const exeName = process.platform === "win32" ? "aivi-lsp.exe" : "aivi-lsp";
  const builtExe = path.join(repoRoot, "target", "release", exeName);
  const outExe = path.join(vscodeDir, "bin", exeName);

  if (!fs.existsSync(builtExe)) {
    throw new Error(`Expected LSP binary at ${builtExe}`);
  }

  copyFile(builtExe, outExe);
  chmodExecutable(outExe);
}

function generateSyntaxes() {
  const outDir = path.join(repoRoot, "vscode", "syntaxes");
  run(`cargo run -p aivi --bin gen_vscode_syntax -- "${outDir}"`, { cwd: repoRoot });
}

function compileExtension() {
  run("pnpm exec tsgo -p .", { cwd: vscodeDir });
}

function packageVsix() {
  run("pnpm exec vsce package", { cwd: vscodeDir });
}

function main() {
  syncIcons();
  generateSyntaxes();
  buildLsp();
  const packageJsonPath = path.join(vscodeDir, "package.json");
  const originalPackageJson = readJson(packageJsonPath);

  let nextVersion = null;
  try {
    nextVersion = bumpPatchVersion(originalPackageJson.version);
    writeJson(packageJsonPath, { ...originalPackageJson, version: nextVersion });
    compileExtension();
    packageVsix();
  } catch (err) {
    writeJson(packageJsonPath, originalPackageJson);
    throw err;
  }

  process.stdout.write(`Built VSIX (version ${nextVersion}).\n`);
}

main();
