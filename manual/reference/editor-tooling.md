# Diagnostics and Editor Tooling

The CLI, language server, and VS Code extension share the same frontend analysis. This page states
their current contract and deliberately separates shipped behavior from possible future work.

## Diagnostic contract

Compiler diagnostics carry:

- a severity: error, warning, note, or help
- an optional stable code in `domain::name` form, such as
  `syntax::unterminated-string` or `hir::type-mismatch`
- one primary source label when a precise location is available
- zero or more secondary labels for related declarations or locations
- notes and actionable help text where the compiler can make a safe suggestion

The CLI renders this structure to stderr. The LSP preserves the code, UTF-16 source range,
severity, and secondary labels as related information. Diagnostics are ordered deterministically
by location, severity, code, and message, so identical source produces stable output.

Editor-only hints use the same naming principle. For example, unused declarations use
`aivi::unused-symbol` in the native diagnostic model and `aivi/unused-symbol` on the LSP wire.

## LSP transport and synchronization

Start the server with:

```sh
aivi lsp
```

Protocol messages use stdin and stdout; logs use stderr. Documents use versioned incremental
synchronization. The server stores text in ropes, applies every edit batch transactionally using
UTF-16 positions, rejects stale versions and invalid ranges, and updates the query database only
after the entire batch validates.

Compiler analysis runs on a bounded worker pool rather than on Tokio executor threads. New edits
cancel superseded diagnostic work, and a final document-version check prevents stale results from
being published. Workspace references and symbols index all `.aivi` files under the initialized workspace roots,
including unopened callers. Open buffers override disk contents. Hidden directories, `target`,
`node_modules`, and symlink directories are excluded. Without workspace roots, discovery uses the
nearest `aivi.toml` ancestor, or the document's parent directory. File-watcher and workspace-folder
notifications refresh the disk snapshot; closing a buffer restores its disk contents. Changes
schedule diagnostics for all open documents so importers do not retain stale errors.

The navigation index is cached by query-database revision and project file set. Discovery and
analysis run on workers; publication remains serialized with document updates.

## Supported LSP capabilities

| Area | Capability |
| --- | --- |
| Synchronization | Incremental open/change/close |
| Feedback | Diagnostics, related information, inlay hints, code actions |
| Editing | Full-document formatting, prepare rename, rename |
| Navigation | Definition, implementation, references, document highlights |
| Discovery | Completion, hover, signature help, document symbols, workspace symbols |
| Structure | Folding ranges, code lenses for `@test` values |
| Highlighting | Full, range, and delta semantic tokens |

Inlay hints and code lenses can be disabled through initialization options. Completion returns current-module declarations, imported names, scoped function/lambda parameters,
and case-pattern bindings. After a dotted record name it returns fields when compiler elaboration
supplies a structural record type; it does not guess fields for opaque or unresolved types and does
not synthesize import edits. Semantic tokens classify resolved type, function, and variable sites,
with lexical highlighting for keywords and literals and TextMate fallback for incomplete code.

Rename emits versioned edits for open buffers and includes unopened callers. It conservatively
requires a fresh identifier in every affected module, a single project-owned target, and unaliased
references. It refuses malformed names, collisions, incomplete disk discovery, erroneous affected
modules, library targets, and record shorthands whose binding rename would also change a field key.
The server reports a rejected rename instead of returning a partial edit. Unused-symbol removal
covers the complete parsed declaration, its attached signature, and decorators; `main` and `@test`
entry points are not treated as unused.

## VS Code extension

Opening an `.aivi` file starts `aivi lsp`. The extension provides syntax and semantic
highlighting, snippets, formatting, diagnostics, navigation, code lenses, and the protocol
capabilities above.

Commands:

| Command | Purpose |
| --- | --- |
| AIVI: Restart Language Server | Serialize a stop/start cycle without overlapping server processes. |
| AIVI: Show Output Channel | Open the extension and server log. |
| AIVI: Format Document | Format the active AIVI document. |
| AIVI: Check Current File | Save and check the active file through a structured process task. |
| AIVI: Run Test | Save workspace files, then run the selected `@test` value or prompt for its name. |

Settings:

| Setting | Default |
| --- | --- |
| `aivi.compiler.path` | `aivi` |
| `aivi.compiler.timeout` | 15000 ms |
| `aivi.diagnostics.debounceMs` | 200 ms |
| `aivi.inlayHints.enabled` | `true` |
| `aivi.inlayHints.maxLength` | 30 |
| `aivi.codeLens.enabled` | `true` |
| `aivi.format.onSave` | `false` |

Multi-root commands run in the workspace folder that owns the active file. File URIs are converted
to native paths, and check/test arguments are passed as literal process arguments rather than
shell text. Protocol tracing uses the standard log level on the **AIVI Trace** output channel.

If startup fails, inspect the **AIVI** output channel and verify `aivi.compiler.path`. A timeout
leaves the extension available, marks the server as crashed, and offers to open the setting.

The extension does not bundle the compiler and does not provide a separate compiler-argument
setting. Those behaviors are not part of the shipped contract.

## Editor verification

`cargo test -p aivi-lsp` covers protocol behavior, edit safety, workspace discovery, and parsing of
all expanded snippet templates. In `tooling`, `pnpm -F vscode-aivi test` runs TypeScript unit and raw
stdio-server integration tests. `pnpm -F vscode-aivi test:host` additionally launches an installed
VS Code in an isolated temporary profile to exercise the actual extension, commands, and restart.
Set `AIVI_VSCODE_EXECUTABLE` to select a VS Code executable; the host test needs a desktop display.
The raw stdio test alone is not extension-host validation.
