# AIVI for VS Code

Official VS Code support for the AIVI language.

## Features

- Syntax, semantic highlighting, and snippets
- Diagnostics with related locations and stable diagnostic codes
- Completion, hover, signature help, go-to-definition, references, rename, and symbols
- Document highlights, folding ranges, inlay hints, code lenses, and code actions
- Full, range, and delta semantic tokens
- Document formatting for `.aivi` files
- An included AIVI editor theme tuned to the extension's syntax colors

Opening an AIVI document starts `aivi lsp` over stdio. Multi-root workspaces use
the owning workspace folder for `check` and `test` task configuration and working
directory. Language-server traffic and process failures are available in the
`AIVI` and `AIVI Trace` output channels.

## Commands

| Command | Purpose |
| --- | --- |
| `AIVI: Restart Language Server` | Stop and restart the serialized language-server lifecycle. |
| `AIVI: Show Output Channel` | Open the AIVI extension and server log. |
| `AIVI: Format Document` | Format the active AIVI document. |
| `AIVI: Check Current File` | Save and check the active file with a structured process task. |
| `AIVI: Run Test` | Run the selected code-lens test, or prompt for a test value. |

`check` and `test` use `ProcessExecution`, so paths and test names are passed as
literal process arguments rather than interpolated shell text. Code-lens
`file://` URIs are converted to native filesystem paths before invocation.

## Settings

| Setting | Default | Purpose |
| --- | --- | --- |
| `aivi.compiler.path` | `aivi` | Compiler executable or absolute executable path. |
| `aivi.compiler.timeout` | `15000` | Language-server startup timeout in milliseconds. |
| `aivi.diagnostics.debounceMs` | `200` | Delay before diagnostics after an edit. |
| `aivi.inlayHints.enabled` | `true` | Enable inferred-type inlay hints. |
| `aivi.inlayHints.maxLength` | `30` | Maximum inlay-hint length. |
| `aivi.codeLens.enabled` | `true` | Enable AIVI code lenses. |
| `aivi.format.onSave` | `false` | Format AIVI documents before saving. |

Changes to compiler or server initialization settings restart the server in a
serialized queue. Format-on-save takes effect without a restart. Protocol
tracing is controlled by the `AIVI Trace` output channel's standard VS Code log
level. If the executable cannot be found or startup times out, the extension
keeps running, marks the status item as crashed, logs the failure, and offers to
open `aivi.compiler.path`.

## License

This VS Code extension is distributed under `GPL-3.0-only`. The full license
text is included in the packaged `licence.txt` file.
