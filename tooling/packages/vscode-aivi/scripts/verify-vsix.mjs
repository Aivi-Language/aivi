import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const manifest = JSON.parse(readFileSync(resolve(packageRoot, "package.json"), "utf8"));
const vsix = resolve(packageRoot, `${manifest.name}-${manifest.version}.vsix`);
const listing = spawnSync("unzip", ["-Z1", vsix], { encoding: "utf8" });
assert.equal(
  listing.status,
  0,
  `could not inspect ${vsix}: ${listing.stderr || listing.error || "unknown unzip failure"}`
);
const files = listing.stdout
  .trim()
  .split("\n");

for (const required of [
  "extension/package.json",
  "extension/dist/extension.js",
  "extension/readme.md",
  "extension/language-configuration.json",
  "extension/syntaxes/aivi.tmLanguage.json",
]) {
  assert.ok(files.includes(required), `VSIX is missing ${required}`);
}
for (const forbidden of ["extension/src/", "extension/tests/", "extension/test-out/"]) {
  assert.ok(
    files.every((file) => !file.startsWith(forbidden)),
    `VSIX unexpectedly contains ${forbidden}`
  );
}

console.log(`verified ${vsix} (${files.length} entries)`);
