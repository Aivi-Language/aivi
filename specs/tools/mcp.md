# MCP Server

`aivi mcp serve` exposes AIVI tooling over the Model Context Protocol (MCP) so editor agents and other local automation can inspect AIVI source targets, read bundled specifications, and, when enabled, inspect or drive GTK applications.

The MCP server is local-first:

- it serves bundled language documentation as MCP resources
- it exposes compiler and formatter tools such as `aivi_parse`, `aivi_check`, and `aivi_fmt`
- with `--ui`, it also exposes `aivi_gtk_*` tools for live GTK inspection and interaction

Tool names are advertised with underscore-safe names (using only letters, numbers, `_`, and `-`, because some MCP hosts reject dotted names in the `tools/list` response). The server still accepts legacy dotted spellings such as `aivi.gtk.launch` on `tools/call` for backwards compatibility.

See also [CLI](cli.md) for the command-line entry point and [`aivi.ui.gtk4`](../stdlib/ui/gtk4.md) for the GTK signal/runtime model behind the UI tools.

## Transport and process model

`aivi mcp serve` is currently a stdio server. Over stdio it accepts newline-delimited JSON-RPC, and it also tolerates LSP-style `Content-Length:` framing for compatibility with MCP hosts that reuse LSP transport code.

The positional `<path|dir/...>` argument in `aivi mcp serve <path|dir/...>` is accepted for CLI compatibility but is currently ignored by the server. Individual MCP tool calls pass their own explicit `target`, `sessionId`, `name`, or `id` arguments.

When `--ui` is enabled, GTK tools use a second local bridge between the MCP server and the running GTK application:

- platform scope is currently Linux / Unix only
- the MCP server launches or connects to a GTK app session
- the GTK app exposes a local Unix socket guarded by a random session token
- requests and responses use newline-delimited JSON

This bridge is intentionally local and per-session. Numeric widget ids are only meaningful within the current GTK session.

## Bundled resources

`aivi mcp serve` always exposes bundled `specs/**/*.md` and `specs/**/*.aivi` files as MCP resources under URIs such as:

```text
aivi://specs/tools/cli.md
aivi://specs/stdlib/ui/gtk4.md
```

This lets MCP clients read the language and tooling documentation without needing to crawl the filesystem separately.

## Core language tools

Without `--ui`, the main MCP tools are:

- `aivi_parse`
- `aivi_check`
- `aivi_fmt`
- `aivi_fmt_write` (requires `--allow-effects`)

These operate on explicit `target` arguments passed per tool call:

- `aivi_parse` parses one file or driver-style target and returns syntax diagnostics
- `aivi_check` type-checks a target and optionally accepts `checkStdlib: true`
- `aivi_fmt` formats exactly one file and returns the formatted text without writing it
- `aivi_fmt_write` formats matching `.aivi` files in place

## GTK session tools

With `--ui`, the server also exposes GTK session management tools:

- `aivi_gtk_discover` — list candidate local GTK debug sockets; it does not reveal the session token needed for attachment
- `aivi_gtk_attach` — connect to an existing GTK session when you already know both the socket path and token
- `aivi_gtk_launch` — start an AIVI GTK application under MCP inspection and return `sessionId`, `socketPath`, `token`, `pid`, and `ready`
- `aivi_gtk_hello` — verify that a session is alive and report high-level capabilities for a known `sessionId`

`aivi_gtk_launch` and all mutation-style GTK tools require `--allow-effects`.

### Session lifecycle

Use either `aivi_gtk_launch` or `aivi_gtk_attach` first. Both return a `sessionId`. Pass that `sessionId` to `aivi_gtk_hello`, the inspection tools, and the interaction tools for the rest of the session.

`aivi_gtk_discover` is only a socket finder. It helps you find candidate sessions, but you still need a valid token before `aivi_gtk_attach` can succeed.

## GTK inspection tools

The first GTK inspection layer is read-only:

- `aivi_gtk_listWidgets`
- `aivi_gtk_inspectWidget`
- `aivi_gtk_dumpTree`

These tools inspect the live reconciled widget tree kept by the GTK runtime. Inspection payloads include:

- widget id
- optional widget name from `id="..."`
- GTK class name
- parent/root relationships
- rendered props captured from the live node
- current dimensions (`width`, `height`)
- runtime state when available, such as text, active/toggled state, and the visible child name for stacks
- supported interaction capabilities (`click`, `type`, `select`, `keyPress`)

### `listWidgets`

Returns a flat list of inspectable widgets for the current session, including dimensions, runtime state, and capability hints.

Use it when you want stable handles quickly.

### `inspectWidget`

Returns the current snapshot for one widget by `name` or numeric `id`.
Window ids reported by `aivi_gtk_hello` can also be inspected by numeric `id`.

This is the preferred tool for targeted inspection because clients do not need to fetch and traverse the whole tree just to read one widget's props or dimensions.

### `dumpTree`

Returns the live widget tree, either for every root or for one `rootId`.

Each node includes its own props, dimensions, runtime state, bound signals, and children.

## GTK interaction tools

The GTK driver layer currently exposes:

- `aivi_gtk_click`
- `aivi_gtk_type`
- `aivi_gtk_select`
- `aivi_gtk_keyPress`

These operate on widgets previously discovered via `listWidgets`, `inspectWidget`, or `dumpTree`. All of them require a `sessionId`; widget selection then happens by `name` or numeric `id`.

### Targeting rules

Widget targeting follows this order:

1. named widget (`name`)
2. numeric widget id (`id`)

If both are supplied, they must refer to the same live widget or the request fails.
`aivi_gtk_keyPress` may omit both `name` and `id`; in that case it targets the sole window when exactly one window is present, otherwise the request fails and the client must choose an explicit target.

### `click`

Dispatches clickable bindings such as `clicked` or `activate` for the target widget and returns the refreshed widget snapshot.

### `type`

Sets text on a text-input widget, emits compatible text-change bindings such as `changed` or `notify::text`, and returns the refreshed widget snapshot.

### `select`

Applies a selectable value to widgets that expose selection-like state, currently:

- stack widgets via `visible-child-name`
- toggle widgets such as `GtkCheckButton` and `AdwSwitchRow` via `active`

The tool returns the refreshed widget snapshot after the selection is applied.

### `keyPress`

Injects a `key-pressed` signal into the target widget or window and returns a receipt plus the refreshed target snapshot.

- `key` is required and maps to the `key` field seen by `GtkKeyPressed`
- `detail` is optional and defaults to `"mcp"`
- omitting `name` and `id` is only valid when the session has exactly one window

This is intended for keyboard-driven GTK apps, such as demos that listen for `GtkKeyPressed` on their window.

## Error behavior

GTK MCP calls fail explicitly when:

- the platform is unsupported
- the socket or token is invalid
- the session has gone away
- the target widget cannot be found
- the requested action is not supported by that widget kind
- request arguments are malformed

There are no silent no-op fallbacks for unsupported widget actions.

## Example workflow

### Launch and inspect a demo app

1. Start an MCP host against `aivi mcp serve . --ui --allow-effects`.
2. Call `aivi_gtk_launch` with:

   ```json
   { "target": "demos/snake.aivi" }
   ```

3. Save the returned `sessionId`, then call `aivi_gtk_hello`:

   ```json
   { "sessionId": "<sessionId>" }
   ```

4. Discover stable widget handles with `aivi_gtk_listWidgets`:

   ```json
   { "sessionId": "<sessionId>" }
   ```

5. Inspect one widget by `name` or numeric `id`:

   ```json
   { "sessionId": "<sessionId>", "name": "<widget-name>" }
   ```

6. Call `aivi_gtk_click`, `aivi_gtk_type`, `aivi_gtk_select`, or `aivi_gtk_keyPress`, then re-read the widget or tree snapshot to observe the updated state.

If the app is already running, use `aivi_gtk_discover` to find candidate sockets, then `aivi_gtk_attach` once you have the matching `socketPath` and `token`.

## Quick verification

A minimal manual smoke test should confirm all of the following:

- `resources/list` includes bundled URIs such as `aivi://specs/tools/mcp.md`
- `tools/list` advertises underscore-safe names such as `aivi_parse` and, with `--ui`, `aivi_gtk_launch`
- `aivi_gtk_launch` on `demos/snake.aivi` returns a `sessionId`
- `aivi_gtk_hello` and `aivi_gtk_listWidgets` succeed when called with that `sessionId`
