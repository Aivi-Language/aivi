import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

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

function toPosix(p) {
  return p.split(path.sep).join("/");
}

function ensureDotSlash(rel) {
  if (rel.startsWith("../") || rel.startsWith("./")) return rel;
  return `./${rel}`;
}

function readJsonFile(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJsonFile(filePath, value) {
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2) + "\n", "utf8");
}

function moduleFromMd(mdRel, blockIndex1) {
  // docs.snippets.02_syntax.09_effects.block_01
  const base = mdRel.replace(/^specs\//, "").replace(/\.md$/i, "");
  const parts = base.split("/").filter(Boolean);
  const safe = parts
    .join(".")
    .replace(/[^a-zA-Z0-9.]+/g, "_")
    .replace(/\.+/g, ".");
  return `docs.snippets.${safe}.block_${String(blockIndex1).padStart(2, "0")}`;
}

function snippetPathFromMd(mdRel, blockIndex1) {
  // specs/snippets/from_md/02_syntax/09_effects/block_01.aivi
  const base = mdRel.replace(/^specs\//, "").replace(/\.md$/i, "");
  const parts = base.split("/").filter(Boolean);
  return path.join(
    "specs",
    "snippets",
    "from_md",
    ...parts,
    `block_${String(blockIndex1).padStart(2, "0")}.aivi`,
  );
}

const argv = process.argv.slice(2);
const apply = argv.includes("--apply");
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
manifest.snippets = Array.isArray(manifest.snippets) ? manifest.snippets : [];
const manifestByPath = new Set(manifest.snippets.map((s) => s?.path).filter(Boolean));

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
if (rgFiles.status !== 0) die(rgFiles.stderr || "rg --files failed");

const mdFiles = rgFiles.stdout
  .trimEnd()
  .split("\n")
  .filter((x) => x.length > 0)
  .sort();

let extracted = 0;
let rewrittenMdFiles = 0;
let addedManifest = 0;

for (const mdRel of mdFiles) {
  const mdAbs = path.resolve(repoRoot, mdRel);
  const original = normalizeNewlines(fs.readFileSync(mdAbs, "utf8"));
  const lines = original.split("\n");

  let blockIndex = 0;
  let changed = false;
  const out = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed === "```aivi") {
      // Extract until closing fence.
      blockIndex++;
      let j = i + 1;
      while (j < lines.length && lines[j].trim() !== "```") j++;
      if (j >= lines.length) {
        die(`unterminated \`\`\`aivi fence in ${mdRel}:${i + 1}`);
      }

      const blockLines = lines.slice(i + 1, j);
      const snippetRel = toPosix(snippetPathFromMd(mdRel, blockIndex));
      const snippetAbs = path.resolve(repoRoot, snippetRel);
      fs.mkdirSync(path.dirname(snippetAbs), { recursive: true });

      // Keep snippet content as-authored; verifier/formatter can normalize later.
      const snippetContent = blockLines.join("\n").replace(/\s+$/g, "").trimEnd() + "\n";

      // Write snippet file if applying; otherwise just simulate.
      if (apply) {
        fs.writeFileSync(snippetAbs, snippetContent, "utf8");
      }

      // Replace the fenced block with a VitePress snippet include.
      const relFromMdDir = ensureDotSlash(
        toPosix(path.relative(path.dirname(mdAbs), snippetAbs)),
      );
      out.push(`<<< ${relFromMdDir}{aivi}`);
      changed = true;
      extracted++;

      // Add to manifest (parse-only by default; check can be enabled later per snippet).
      if (!manifestByPath.has(snippetRel)) {
        manifest.snippets.push({
          path: snippetRel,
          module: moduleFromMd(mdRel, blockIndex),
          verify: ["fmt", "parse"],
        });
        manifestByPath.add(snippetRel);
        addedManifest++;
      }

      i = j; // skip closing fence
      continue;
    }

    out.push(line);
  }

  if (changed) {
    rewrittenMdFiles++;
    if (apply) {
      fs.writeFileSync(mdAbs, out.join("\n"), "utf8");
    }
  }
}

if (apply) {
  writeJsonFile(manifestPath, manifest);
}

process.stdout.write(
  JSON.stringify(
    {
      apply,
      markdownFilesScanned: mdFiles.length,
      markdownFilesRewritten: rewrittenMdFiles,
      snippetsExtracted: extracted,
      manifestEntriesAdded: addedManifest,
    },
    null,
    2,
  ) + "\n",
);

