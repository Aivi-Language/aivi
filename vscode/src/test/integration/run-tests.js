/* eslint-disable no-console */
const path = require('node:path');
const fs = require('node:fs');
const { runTests } = require('@vscode/test-electron');

function firstExistingPath(paths) {
  for (const p of paths) {
    if (!p) continue;
    try {
      if (fs.existsSync(p)) return p;
    } catch {
      // ignore
    }
  }
  return undefined;
}

async function main() {
  const extensionDevelopmentPath = path.resolve(__dirname, '../../..');
  const extensionTestsPath = path.resolve(__dirname, './suite/index.js');
  const fixtureWorkspacePath = path.resolve(
    extensionDevelopmentPath,
    'test-fixtures',
    'lsp-html'
  );

  const vscodeExecutablePath = firstExistingPath([process.env.VSCODE_EXECUTABLE_PATH]);

  // If no local VS Code is found, @vscode/test-electron will attempt to download one.
  // (CI typically allows this; local dev can set VSCODE_EXECUTABLE_PATH to stay offline.)
  await runTests({
    vscodeExecutablePath,
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [
      fixtureWorkspacePath,
      '--disable-extensions',
      '--skip-welcome',
      '--skip-release-notes',
      '--disable-workspace-trust',
    ],
  });
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
