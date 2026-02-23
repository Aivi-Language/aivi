# VSCode Extension

The **AIVI Language Support** extension for Visual Studio Code provides a comprehensive development environment for AIVI.

## Features

- **Syntax Highlighting**: Robust TextMate-based grammar for `.aivi` files with HTML injection support for AIVI sigils containing HTML.
- **Semantic Highlighting**: Custom semantic token types for fine-grained coloring (arrows, pipes, brackets, dots, units, sigils, path segments).
- **LSP Integration**: Automatically starts the `aivi lsp` server to provide advanced features like:
  - Go to Definition / Declaration / Implementation / References
  - Hover information
  - Autocomplete
  - Signature Help
  - Renaming
  - Code Actions
  - Real-time Error Reporting
- **Formatting**: Integrated formatter support (`Shift + Alt + F` or Format on Save).
- **Test Runner**: Run AIVI `@test` definitions from the editor via commands.
- **Theme**: Bundled "AIVI Dark Modern" color theme optimized for AIVI syntax.
- **EBNF Support**: Syntax highlighting for `.ebnf` files (used in AIVI development).

## Commands

| Command               | Title                          | Description                                    |
|:--------------------- |:------------------------------ |:---------------------------------------------- |
| `aivi.restartServer`  | AIVI: Restart Language Server  | Restarts the LSP server.                       |
| `aivi.runTests`       | AIVI: Run Tests                | Runs all `@test` definitions in the workspace. |
| `aivi.runTestsFile`   | AIVI: Run Tests (Current File) | Runs tests in the current `.aivi` file.        |
| `aivi.runTestsFolder` | AIVI: Run Tests (Folder)       | Runs tests in a selected folder.               |

## Configuration

This extension contributes the following settings to VSCode:

### Server Settings

| Setting               | Type     | Default | Description                                                                                                                             |
|:--------------------- |:-------- |:------- |:--------------------------------------------------------------------------------------------------------------------------------------- |
| `aivi.server.command` | string   | `""`    | Override the language server command. If unset, the extension prefers the bundled `aivi-lsp`, then `aivi-lsp` on PATH, then `aivi lsp`. |
| `aivi.server.args`    | string[] | `[]`    | Extra arguments for `aivi.server.command`.                                                                                              |

### Format Settings

| Setting                     | Type   | Default | Description                                                               |
|:--------------------------- |:------ |:------- |:------------------------------------------------------------------------- |
| `aivi.format.indentSize`    | number | `2`     | Number of spaces to use for indentation (1–8).                            |
| `aivi.format.maxBlankLines` | number | `1`     | Maximum number of consecutive blank lines allowed by the formatter (0–5). |

### CLI Settings

| Setting            | Type   | Default  | Description                                                   |
|:------------------ |:------ |:-------- |:------------------------------------------------------------- |
| `aivi.cli.command` | string | `"aivi"` | Command used to invoke the AIVI CLI (used for running tests). |

## Semantic Token Types

The extension registers custom semantic token types for precise highlighting:

| Token Type     | Super Type | Description                         |
|:-------------- |:---------- |:----------------------------------- |
| `aiviArrow`    | `operator` | Arrow operators (`=>`, `<-`, `->`)  |
| `aiviPipe`     | `operator` | Pipe operators (`\|>`, `<\|`, `\|`) |
| `aiviBracket`  | `operator` | Brackets and parentheses            |
| `aiviDot`      | `operator` | Dot separator in dotted paths       |
| `aiviUnit`     | `keyword`  | Domain unit suffixes                |
| `aiviSigil`    | `string`   | Sigil prefix markers                |
| `aiviPathHead` | `variable` | First segment of a dotted path      |
| `aiviPathMid`  | `variable` | Middle segment of a dotted path     |
| `aiviPathTail` | `variable` | Last segment of a dotted path       |

## Installation

The extension is currently distributed as a `.vsix` file. You can install it manually in VSCode:

1. Open the Command Palette (`Ctrl+Shift+P`).
2. Type "Extensions: Install from VSIX...".
3. Select the `aivi-vscode-x.y.z.vsix` file.

Distribution is currently via `.vsix`; Marketplace publication is out of scope for v0.1.
