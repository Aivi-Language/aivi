# AIVI VS Code Extension

Language support for **AIVI**:

- Syntax highlighting (TextMate grammar)
- Basic code formatting (`Format Document`)

## Develop

```bash
cd vscode
pnpm install
pnpm run compile
```

Press `F5` in VS Code to launch an Extension Development Host.

## Package

```bash
cd vscode
pnpm install
pnpm run build
```

This produces a `.vsix` in `vscode/`.

`pnpm run build` also rebuilds `aivi-lsp` and regenerates `vscode/syntaxes/*.tmLanguage.json` from Rust.
