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
being published. Workspace references and symbols use one immutable index per query-database
revision instead of rescanning the workspace for every request.

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

Inlay hints and code lenses can be disabled through initialization options. Completion currently
returns compiler-backed names and member suggestions but does not synthesize import edits.

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
| AIVI: Run Test | Run the selected `@test` value, or prompt for an exact value name. |

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
