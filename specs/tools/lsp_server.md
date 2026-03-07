# LSP Server

The AIVI Language Server (`aivi-lsp`) implements the Language Server Protocol so editors such as VS Code, Neovim, Zed, and other LSP clients can understand AIVI code.

In practice, the language server is what powers “editor intelligence”: navigation, hover information, diagnostics, formatting, and code-aware completion.

## What the server provides

### Navigation

- **Go to Definition**: jump to where a symbol is defined.
- **Go to Declaration**: behaves the same as definition for most items.
- **Go to Implementation**: jump to implementations of classes.
- **Find References**: list uses of a symbol across the workspace.
- **Document Symbols**: show an outline of symbols in the current file.

### Information and help while editing

- **Hover**: shows type information and documentation. The server first tries definition-based lookup, then falls back to span-type lookup so hover still works for expressions that do not have a named definition.
- **Signature Help**: shows function signatures and parameter information while you type.
- **Schema and source help**: hover content also documents source-oriented helpers such as `file.json`, `env.decode`, `source.transform`, `source.validate`, `source.decodeErrors`, and `source.schema.derive`.
- **GTK architecture help**: hover content documents `gtkApp`, `appStep`, `noSubscriptions`, `commandAfter`, `commandPerform`, `subscriptionEvery`, and `subscriptionSource`.

### Editing support

- **Completion**: context-aware completion for keywords, variables, functions, and types.
- **Rename**: rename a symbol and its references across the workspace.
- **Code Actions**: context-sensitive fixes and refactorings based on diagnostics.
- **Semantic Tokens**: token-aware highlighting that distinguishes language concepts more precisely than plain text grammars can.
- **Formatting**: document formatting through the built-in formatter.

Completion also includes scaffolds for the recommended `gtkApp` architecture so editors guide users toward the main public GTK app loop.

## Diagnostics

The server reports diagnostics as you type. It checks:

- syntax
- types
- scope and name resolution
- structured-source ergonomics, including legacy source forms and missing schema strategies
- GTK and app-architecture ergonomics, including the recommended `gtkApp` host and the standard command and subscription helpers

## Incremental workspace checking

The LSP works with **workspace snapshots**, not isolated one-file analyses.

- open documents shadow on-disk files for that snapshot
- older in-flight semantic work may be cancelled or superseded
- cached module facts may be reused only when their fingerprints match the active snapshot
- dependent modules are rechecked incrementally when an edited module's export surface changes

The full ownership, invalidation, and publish rules are defined in [Incremental Compilation & Workspace Checking](incremental_compilation.md).

## Configuration

The server reads configuration from editor settings sent via `workspace/didChangeConfiguration`.

### Format options

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `format.indentSize` | number | `2` | Number of spaces for indentation. |
| `format.maxBlankLines` | number | `1` | Maximum consecutive blank lines. |
| `format.braceStyle` | string | `"kr"` | Brace placement style: `"kr"` or `"allman"`. |
| `format.maxWidth` | number | `100` | Maximum line width before wrapping. |

### Diagnostics options

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `diagnostics.includeSpecsSnippets` | boolean | `false` | Include diagnostics for AIVI snippets embedded in spec markdown files. |

### Strict mode

| Setting | Type | Default | Description |
|:-------- |:---- |:------- |:----------- |
| `strict.level` | number | `0` | Strictness level (`0` = default, higher = stricter). |
| `strict.forbidImplicitCoercions` | boolean | `false` | Forbid implicit type coercions. |
| `strict.warningsAsErrors` | boolean | `false` | Treat warnings as errors. |

## Starting the server

The `aivi` CLI can launch the server directly, and editor integrations usually do exactly that:

```bash
aivi lsp  # Start the language server for an editor client.
```

The server is also available as the standalone binary `aivi-lsp`.
