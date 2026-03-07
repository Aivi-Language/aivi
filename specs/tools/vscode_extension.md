# VSCode Extension

The **AIVI Language Support** extension turns Visual Studio Code into an AIVI-aware editor. It combines grammar-based syntax highlighting, language-aware semantic tokens, formatter integration, test commands, and the AIVI language server.

## What the extension adds

- **Syntax highlighting** for `.aivi` files, including HTML/GTK injection support and embedded-language highlighting inside tagged raw-text sigils such as `` ~`css color: red;` ``.
- **Semantic highlighting** with custom token types for arrows, pipes, brackets, dots, units, sigils, and dotted-path segments. This is extra language-aware coloring on top of the grammar-based highlighting.
- **Language server integration** so features such as hover, rename, go-to-definition, diagnostics, and completion work automatically.
- **Formatting** through the built-in formatter (`Shift + Alt + F` or Format on Save).
- **Test commands** for running AIVI `@test` definitions from the editor.
- **Theme support** through the bundled “AIVI Dark Modern” color theme.
- **EBNF syntax highlighting** for `.ebnf` files used in AIVI development.

## Editor commands

| Command | Title | What it does |
|:-------- |:----- |:------------ |
| `aivi.restartServer` | AIVI: Restart Language Server | Restarts the LSP server used by the extension. |
| `aivi.runTests` | AIVI: Run Tests | Runs all `@test` definitions in the workspace. |
| `aivi.runTestsFile` | AIVI: Run Tests (Current File) | Runs tests in the current `.aivi` file. |
| `aivi.runTestsFolder` | AIVI: Run Tests (Folder) | Runs tests in a selected folder. |

## Configuration

### Server settings

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `aivi.server.command` | string | `""` | Override the language server command. If unset, the extension prefers the bundled `aivi-lsp`, then `aivi-lsp` on `PATH`, then `aivi lsp`. |
| `aivi.server.args` | string[] | `[]` | Extra arguments for `aivi.server.command`. |

### Format settings

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `aivi.format.indentSize` | number | `2` | Number of spaces used for indentation (`1`–`8`). |
| `aivi.format.maxBlankLines` | number | `1` | Maximum number of consecutive blank lines allowed by the formatter (`0`–`5`). |

### CLI settings

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `aivi.cli.command` | string | `"aivi"` | Command used to invoke the AIVI CLI for test-related editor actions. |

## Semantic token types

The extension registers custom semantic token types so themes can style AIVI constructs more precisely.

| Token Type | Super Type | Description |
|:---------- |:---------- |:----------- |
| `aiviArrow` | `operator` | Arrow operators such as `=>`, `<-`, and `->` |
| `aiviPipe` | `operator` | Pipe operators such as `|>`, `<|`, and `|` |
| `aiviBracket` | `operator` | Brackets and parentheses |
| `aiviDot` | `operator` | Dot separators in dotted paths |
| `aiviUnit` | `keyword` | Domain unit suffixes |
| `aiviSigil` | `string` | Sigil prefix markers |
| `aiviPathHead` | `variable` | First segment of a dotted path |
| `aiviPathMid` | `variable` | Middle segment of a dotted path |
| `aiviPathTail` | `variable` | Last segment of a dotted path |

## Installation

The extension is distributed as a `.vsix` package.

1. Open the Command Palette (`Ctrl+Shift+P`).
2. Run **Extensions: Install from VSIX...**.
3. Select the `aivi-vscode-x.y.z.vsix` file.

This distribution path keeps installation simple even when the extension is not being published through the Marketplace.
