# VS Code Extension Testing

This folder has two test tiers:

## Unit (Node-only)

Runs Vitest suites in plain Node (no VS Code instance). Includes TextMate tokenization + HTML injection
tests for the AIVI grammar.

```bash
cd vscode
pnpm run test:unit
```

## Integration (VS Code + bundled LSP)

Launches VS Code with the extension under test and exercises LSP completions inside `~<html>...</html>` regions.

```bash
cd vscode
pnpm run test:integration
```

Run both:

```bash
cd vscode
pnpm run test:all
```

Requirements:

- A bundled LSP binary at `vscode/bin/aivi-lsp` (or `aivi-lsp.exe` on Windows).
  Build it with:

  ```bash
  cd vscode
  pnpm run build
  ```

- VS Code executable:
  - If `VSCODE_EXECUTABLE_PATH` is set, it will be used.
  - Otherwise, `@vscode/test-electron` will try to download VS Code (CI-friendly, but requires network).
