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

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

// Use ripgrep for speed and consistent line numbers.
const rg = run(
  "rg",
  ["-n", "--no-heading", "--glob", "*.md", "```aivi", "specs"],
  { cwd: repoRoot },
);
if (rg.status !== 0 && rg.status !== 1) {
  die(rg.stderr || "rg failed");
}

const matches = rg.stdout
  .trimEnd()
  .split("\n")
  .filter((x) => x.length > 0)
  .map((line) => {
    const firstColon = line.indexOf(":");
    const secondColon = line.indexOf(":", firstColon + 1);
    const file = line.slice(0, firstColon);
    const lineNo = Number(line.slice(firstColon + 1, secondColon));
    return { file, lineNo };
  });

const rgIncludes = run(
  "rg",
  ["-n", "--no-heading", "--glob", "*.md", "^\\s*<<<\\s+.*\\.aivi\\b", "specs"],
  { cwd: repoRoot },
);
if (rgIncludes.status !== 0 && rgIncludes.status !== 1) {
  die(rgIncludes.stderr || "rg (includes) failed");
}
const includeLines = rgIncludes.stdout
  .trimEnd()
  .split("\n")
  .filter((x) => x.length > 0);

function readLines(filePath) {
  return fs.readFileSync(path.resolve(repoRoot, filePath), "utf8").split(/\r?\n/);
}

let totalBlocks = 0;
let alreadyExternalized = 0;
const perFile = new Map();

for (const { file, lineNo } of matches) {
  const lines = readLines(file);
  const startIdx = lineNo - 1;
  let endIdx = startIdx + 1;
  while (endIdx < lines.length && lines[endIdx].trim() !== "```") endIdx++;
  const blockLines = lines.slice(startIdx + 1, endIdx);
  totalBlocks++;

  const hasInclude = blockLines.some((l) => l.trimStart().startsWith("<<< "));
  if (hasInclude) alreadyExternalized++;

  const entry = perFile.get(file) ?? [];
  entry.push({
    start: lineNo,
    end: endIdx + 1,
    include: hasInclude,
    preview: blockLines.find((l) => l.trim().length > 0)?.trim() ?? "",
  });
  perFile.set(file, entry);
}

const files = [...perFile.keys()].sort();
process.stdout.write(
  JSON.stringify(
    {
      files: files.length,
      blocks: totalBlocks,
      blocksAlreadyExternalized: alreadyExternalized,
      blocksInline: totalBlocks - alreadyExternalized,
      includeDirectives: includeLines.length,
      byFile: Object.fromEntries(files.map((f) => [f, perFile.get(f)])),
    },
    null,
    2,
  ) + "\n",
);
