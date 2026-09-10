import { mkdtempSync, mkdirSync, writeFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const root = fileURLToPath(new URL("..", import.meta.url));
const temp = mkdtempSync(resolve(tmpdir(), "aivi-extension-host-"));
const project = resolve(temp, "project");
mkdirSync(resolve(project, ".vscode"), { recursive: true });
writeFileSync(resolve(project, "aivi.toml"), "");
writeFileSync(resolve(project, "main.aivi"), "type Int -> Int\nfunc identity = input =>\n    input\n\n@test\nvalue selected : Task Text Bool = pure True\n");
writeFileSync(resolve(project, ".vscode/settings.json"), JSON.stringify({
  "aivi.compiler.path": resolve(root, "../../../target/debug/aivi"),
  "aivi.compiler.timeout": 30000,
}));
const child = spawn(process.env.AIVI_VSCODE_EXECUTABLE || "code", [
  "--new-window", "--wait", "--verbose", "--disable-extensions", "--disable-workspace-trust", "--skip-welcome", "--skip-release-notes",
  `--user-data-dir=${resolve(temp, "profile")}`, `--extensions-dir=${resolve(temp, "extensions")}`,
  `--extensionDevelopmentPath=${root}`, `--extensionTestsPath=${resolve(root, "test-out/tests/extension.host.js")}`,
  project,
], { stdio: "inherit" });
const timeout = setTimeout(() => child.kill(), 120_000);
try {
  const code = await new Promise((resolveExit, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => signal ? reject(new Error(`VS Code terminated: ${signal}`)) : resolveExit(code));
  });
  if (code !== 0 || !existsSync(resolve(project, ".host-test-passed"))) {
    throw new Error(`VS Code host did not complete the tests (exit ${code})`);
  }
  console.log("VS Code extension-host tests passed");
} finally {
  clearTimeout(timeout);
  rmSync(temp, { recursive: true, force: true });
}
