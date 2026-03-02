import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

function run(cmd, args, { cwd }) {
  const result = spawnSync(cmd, args, { cwd, encoding: "utf8" });
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function normalizeNewlines(text) {
  return text.replace(/\r\n/g, "\n");
}

function isIncludeLine(line) {
  return /^\s*<<<\s+.*\.aivi\b/.test(line);
}

const argv = process.argv.slice(2);
const apply = argv.includes("--apply");

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const rgFiles = run(
  "rg",
  [
    "--files",
    "--glob",
    "*.md",
    "--glob",
    "!specs/node_modules/**",
    "--glob",
    "!specs/.vitepress/**",
    "specs",
  ],
  { cwd: repoRoot },
);
if (rgFiles.status !== 0) {
  process.stderr.write(rgFiles.stderr || "rg --files failed\n");
  process.exit(1);
}

const mdFiles = rgFiles.stdout
  .trimEnd()
  .split("\n")
  .filter((x) => x.length > 0)
  .sort();

let changedFiles = 0;
let includeLinesTouched = 0;

for (const mdRel of mdFiles) {
  const mdAbs = path.resolve(repoRoot, mdRel);
  const original = normalizeNewlines(fs.readFileSync(mdAbs, "utf8"));
  const lines = original.split("\n");

  const out = [];
  let changed = false;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (!isIncludeLine(line)) {
      out.push(line);
      continue;
    }

    // Ensure the include directive is its own block:
    // - blank line before
    // - blank line after
    const prev = out.length > 0 ? out[out.length - 1] : "";
    if (prev.trim() !== "") {
      out.push("");
      changed = true;
      includeLinesTouched++;
    }

    out.push(line.trimEnd());

    const next = i + 1 < lines.length ? lines[i + 1] : "";
    if (next.trim() !== "") {
      out.push("");
      changed = true;
      includeLinesTouched++;
    }
  }

  if (changed) {
    changedFiles++;
    if (apply) {
      fs.writeFileSync(mdAbs, out.join("\n"), "utf8");
    }
  }
}

process.stdout.write(
  JSON.stringify(
    { apply, markdownFilesChanged: changedFiles, includeSeparatorsInserted: includeLinesTouched },
    null,
    2,
  ) + "\n",
);

