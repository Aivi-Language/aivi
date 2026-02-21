# LSP Server

The AIVI Language Server (`aivi-lsp`) implements the Language Server Protocol (LSP) to provide rich editing features in compatible editors (like VSCode, Neovim, Zed, etc.).

## Capabilities

The server currently supports the following LSP capabilities:

### Navigation

-   **Go to Definition**: Jump to where a symbol (function, type, variable) is defined.
-   **Go to Declaration**: Same as definition for most items.
-   **Go to Implementation**: Navigate to implementations of classes.
-   **Find References**: List all usages of a symbol in the workspace.
-   **Document Symbols**: List all symbols defined in the current file (outline view).

### Information

-   **Hover**: Show type information and documentation when hovering over a symbol.
-   **Signature Help**: Show function signature and parameter information while typing function calls.

### Editing

-   **Completion**: Context-aware code completion for keywords, variables, functions, and types.
-   **Rename**: Rename a symbol and all its references across the workspace.
-   **Code Actions**: Contextual fixes and refactorings (based on diagnostics).
-   **Semantic Tokens**: Semantic syntax highlighting for precise coloring of tokens (e.g., distinguishing types from variables).
-   **Formatting**: Format the document using the built-in formatter.

## Diagnostics

The LSP server reports diagnostics (errors and warnings) in real-time as you type. It performs:
-   **Syntax Checking**: Validates the grammar of the code.
-   **Type Checking**: Ensures type safety and correctness.
-   **Scope Analysis**: Checks for undefined variables and scoping rules.

## Configuration

The LSP server reads configuration from the editor's settings (sent via `workspace/didChangeConfiguration`). The following sections are supported:

### Format Options

| Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `format.indentSize` | number | `2` | Number of spaces for indentation. |
| `format.maxBlankLines` | number | `1` | Maximum consecutive blank lines. |
| `format.braceStyle` | string | `"kr"` | Brace placement style: `"kr"` (K&R / Java / TS) or `"allman"`. |
| `format.maxWidth` | number | `100` | Maximum line width before wrapping. |

### Diagnostics Options

| Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `diagnostics.includeSpecsSnippets` | boolean | `false` | Include diagnostics for AIVI snippets embedded in specification markdown files. |

### Strict Mode

| Setting | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `strict.level` | number | `0` | Strictness level (0 = default, higher = stricter). |
| `strict.forbidImplicitCoercions` | boolean | `false` | Forbid implicit type coercions. |
| `strict.warningsAsErrors` | boolean | `false` | Treat warnings as errors. |

## Installation & Usage

The LSP server is embedded in the `aivi` CLI but can also be run as a standalone binary `aivi-lsp`.

Editors typically start it via:

```bash
aivi lsp
```
