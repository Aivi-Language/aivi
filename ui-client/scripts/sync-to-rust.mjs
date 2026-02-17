import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(new URL(".", import.meta.url).pathname, "..", "..");
const dist = path.join(repoRoot, "ui-client", "dist", "aivi-server-html-client.js");

if (!fs.existsSync(dist)) {
  console.error(`Missing build output: ${dist}`);
  process.exit(1);
}

const out = fs.readFileSync(dist, "utf8");

const targets = [
  path.join(repoRoot, "crates", "aivi", "src", "runtime", "builtins", "ui", "server_html_client.js"),
  path.join(repoRoot, "crates", "aivi_native_runtime", "src", "builtins", "ui", "server_html_client.js")
];

for (const t of targets) {
  fs.writeFileSync(t, out, "utf8");
  console.log(`Wrote ${t}`);
}

