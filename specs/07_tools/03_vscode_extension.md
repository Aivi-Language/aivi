# VSCode Extension

The **AIVI Language Support** extension for Visual Studio Code provides a comprehensive development environment for Aivi.

## Features

-   **Syntax Highlighting**: Robust TextMate-based grammar for `.aivi` files.
-   **LSP Integration**: Automatically starts the `aivi lsp` server to provide advanced features like:
    -   Go to Definition / References
    -   Hover information
    -   Autocomplete
    -   Renaming
    -   Real-time Error Reporting
-   **Formatting**: Integrated formatter support (`Shift + Alt + F` or Format on Save).
-   **EBNF Support**: Syntax highlighting for `.ebnf` files (used in Aivi development).

## Configuration

This extension contributes the following settings to VSCode:

| Setting | Default | Description |
| :--- | :--- | :--- |
| `aivi.format.indentSize` | `2` | Number of spaces to use for indentation. |
| `aivi.format.maxBlankLines` | `1` | Maximum number of consecutive blank lines allowed by the formatter. |

## Installation

The extension is currently distributed as a `.vsix` file. You can install it manually in VSCode:

1.  Open the Command Palette (`Ctrl+Shift+P`).
2.  Type "Extensions: Install from VSIX...".
3.  Select the `aivi-vscode-x.y.z.vsix` file.

(Roadmap: Publish to Visual Studio Marketplace)
