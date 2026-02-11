# LSP Server

The Aivi Language Server (`aivi-lsp`) implements the Language Server Protocol (LSP) to provide rich editing features in compatible editors (like VSCode, Neovim, Zed, etc.).

## Capabilities

The server currently supports the following LSP capabilities:

### Navigation

-   **Go to Definition**: Jump to where a symbol (function, type, variable) is defined.
-   **Go to Declaration**: Same as definition for most items.
-   **Go to Implementation**: Navigate to implementations of classes or traits.
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

## Installation & Usage

The LSP server is embedded in the `aivi` CLI but can also be run as a standalone binary `aivi-lsp`.

Editors typically start it via:

```bash
aivi lsp
```
