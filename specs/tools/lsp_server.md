# LSP Server

The AIVI Language Server (`aivi-lsp`) implements the Language Server Protocol so editors such as VS Code, Neovim, Zed, and other LSP clients can understand AIVI code.

In practice, the language server is what powers “editor intelligence”: navigation, hover information, diagnostics, formatting, and code-aware completion. If you mainly want the user-facing picture, the first half of this page is enough; later sections are more contributor-facing and focus on cache ownership and incremental rechecking.

## Start here

If you only need the practical picture, read these sections in order:

1. [Mental model](#mental-model)
2. [What the server provides](#what-the-server-provides)
3. [Diagnostics](#diagnostics)
4. [Starting the server](#starting-the-server)

You can treat the rest as implementation detail unless you are debugging editor integration or incremental-checking behavior.

## Mental model

If you just want the practical picture, think of the LSP server as the background process your editor talks to while you type:

- the editor sends the current file contents, including unsaved changes
- the server parses, resolves, and type-checks enough of the workspace to answer the request
- the editor shows the result as hovers, diagnostics, completion items, formatting, and navigation

In a typical edit loop, the flow is:

1. you type in a file
2. the editor sends the newest text to the server
3. the server rechecks the necessary slice of the workspace
4. the editor updates squiggles, hovers, completions, and navigation data

When this page mentions a **workspace snapshot**, it means the full per-request view of the workspace: open editor buffers, on-disk files, and active settings taken together.

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

- **syntax** — catches parse errors and malformed code early
- **types** — catches type mismatches before runtime
- **scope and name resolution** — catches missing imports, unknown names, and similar wiring mistakes
- **structured-source ergonomics** — warns about source-boundary patterns such as legacy source forms or missing schema strategies
- **GTK and app-architecture ergonomics** — guides users toward the recommended `gtkApp` host and standard command and subscription helpers

## Incremental workspace checking

The LSP works with **workspace snapshots**, not isolated one-file analyses.

- open documents shadow on-disk files for that snapshot
- older in-flight semantic work may be cancelled or superseded
- cached module facts may be reused only when their fingerprints match the active snapshot
- dependent modules are rechecked incrementally when an edited module's export surface changes

One concrete example:

- if you edit a private helper inside `Module A`, the server usually rechecks `Module A` but can keep importers clean
- if you change a public type or exported function in `Module A`, importing modules also need to be rechecked
- if you have unsaved changes in the editor, those changes belong only to the current LSP session and must not leak into persistent build caches

The full ownership, invalidation, and publish rules are defined in [Incremental Compilation & Workspace Checking](incremental_compilation.md), which explains how the server reuses cached work without letting unsaved edits leak into persistent build state.

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
