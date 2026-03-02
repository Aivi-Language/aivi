import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function run(cmd, args, { cwd, env }) {
  const result = spawnSync(cmd, args, {
    cwd,
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function normalizeNewlines(text) {
  return text.replace(/\r\n/g, "\n");
}

function readJsonFile(filePath) {
  const raw = fs.readFileSync(filePath, "utf8");
  return JSON.parse(raw);
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function safeTempName(relativePath) {
  return relativePath.replace(/[^a-zA-Z0-9._-]+/g, "__");
}

const argv = process.argv.slice(2);
const fix = argv.includes("--fix");
const manifestArgIndex = argv.indexOf("--manifest");
const manifestRel =
  manifestArgIndex === -1 ? "specs/snippets/manifest.json" : argv[manifestArgIndex + 1];
if (!manifestRel) die("missing value for --manifest");

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const manifestPath = path.resolve(repoRoot, manifestRel);

if (!fs.existsSync(manifestPath)) die(`manifest not found: ${manifestRel}`);
const manifest = readJsonFile(manifestPath);
if (manifest?.version !== 1) die(`unsupported manifest version: ${manifest?.version}`);

const snippets = Array.isArray(manifest.snippets) ? manifest.snippets : [];
if (snippets.length === 0) die("no snippets found in manifest");

const tmpRoot = path.join(os.tmpdir(), "aivi-doc-snippets");
ensureDir(tmpRoot);

let failures = 0;

for (const entry of snippets) {
  const relPath = entry?.path;
  if (typeof relPath !== "string") {
    failures++;
    process.stderr.write(`invalid snippet entry (missing path)\n`);
    continue;
  }

  const verify = Array.isArray(entry.verify) ? entry.verify : ["fmt", "parse"];
  const snippetPath = path.resolve(repoRoot, relPath);
  if (!fs.existsSync(snippetPath)) {
    failures++;
    process.stderr.write(`missing snippet file: ${relPath}\n`);
    continue;
  }

  const rawSnippet = normalizeNewlines(fs.readFileSync(snippetPath, "utf8"));

  const stdlibMode = typeof entry.stdlib === "string" ? entry.stdlib : "embedded";
  const env =
    stdlibMode === "none" ? { ...process.env, AIVI_NO_STDLIB: "1" } : { ...process.env };

  if (verify.includes("fmt")) {
    const fmt = run(
      "cargo",
      ["run", "-q", "-p", "aivi", "--bin", "aivi", "--", "fmt", snippetPath],
      { cwd: repoRoot, env },
    );
    if (fmt.status !== 0) {
      failures++;
      process.stderr.write(`fmt failed: ${relPath}\n${fmt.stderr || fmt.stdout}\n`);
    } else {
      const formatted = normalizeNewlines(fmt.stdout);
      if (formatted !== rawSnippet) {
        if (fix) {
          fs.writeFileSync(snippetPath, formatted, "utf8");
          process.stdout.write(`fixed fmt: ${relPath}\n`);
        } else {
          failures++;
          process.stderr.write(`needs fmt: ${relPath} (run pnpm -C specs snippets:fix)\n`);
        }
      }
    }
  }

  const wantsParse = verify.includes("parse");
  const wantsCheck = verify.includes("check");

  if (wantsParse || wantsCheck) {
    const moduleName = typeof entry.module === "string" ? entry.module : null;
    const uses = Array.isArray(entry.uses) ? entry.uses : [];
    const prelude = entry.prelude;
    const preludePaths = Array.isArray(prelude)
      ? prelude
      : typeof prelude === "string"
        ? [prelude]
        : [];

    const tempFile = path.join(tmpRoot, `${safeTempName(relPath)}.harness.aivi`);

    let harness = "";
    if (moduleName) harness += `module ${moduleName}\n\n`;
    for (const useLine of uses) harness += `use ${useLine}\n`;
    if (uses.length > 0) harness += "\n";
    for (const preludeRel of preludePaths) {
      const preludePath = path.resolve(repoRoot, preludeRel);
      if (!fs.existsSync(preludePath)) {
        failures++;
        process.stderr.write(`missing prelude: ${preludeRel} (for ${relPath})\n`);
        continue;
      }
      harness += normalizeNewlines(fs.readFileSync(preludePath, "utf8")).trimEnd();
      harness += "\n\n";
    }
    harness += rawSnippet.trimEnd();
    harness += "\n";

    fs.writeFileSync(tempFile, harness, "utf8");

    if (wantsParse) {
      const parsed = run(
        "cargo",
        ["run", "-q", "-p", "aivi", "--bin", "aivi", "--", "parse", tempFile],
        { cwd: repoRoot, env },
      );
      if (parsed.status !== 0) {
        failures++;
        process.stderr.write(`parse failed: ${relPath}\n${parsed.stderr || parsed.stdout}\n`);
      }
    }

    if (wantsCheck) {
      const checked = run(
        "cargo",
        ["run", "-q", "-p", "aivi", "--bin", "aivi", "--", "check", tempFile],
        { cwd: repoRoot, env },
      );
      if (checked.status !== 0) {
        failures++;
        process.stderr.write(`check failed: ${relPath}\n${checked.stderr || checked.stdout}\n`);
      }
    }
  }
}

if (failures > 0) process.exit(1);
