# MCP Server

`aivi mcp serve` exposes AIVI tooling over the Model Context Protocol (MCP) so editor agents and other local automation can inspect source code, read bundled documentation, and, when enabled, inspect or drive GTK applications.

The MCP server is local-first:

- it serves bundled language documentation as MCP resources
- it exposes compiler and formatter tools such as `aivi.parse`, `aivi.check`, and `aivi.fmt`
- with `--ui`, it also exposes `aivi.gtk.*` tools for live GTK inspection and interaction

## Transport and process model

The MCP server itself speaks JSON-RPC over the MCP transport chosen by the client, typically stdio.

When `--ui` is enabled, GTK tools use a second local bridge between the MCP server and the running GTK application:

- platform scope is currently Linux / Unix only
- the MCP server launches or connects to a GTK app session
- the GTK app exposes a local Unix socket guarded by a random session token
- requests and responses use newline-delimited JSON

This bridge is intentionally local and per-session. Numeric widget ids are only meaningful within the current GTK session.

## Bundled resources

`aivi mcp serve` always exposes bundled specs as MCP resources under URIs such as:

```text
aivi://specs/tools/cli.md
aivi://specs/stdlib/ui/gtk4.md
```

This lets MCP clients read the language and tooling documentation without needing to crawl the filesystem separately.

## Core language tools

Without `--ui`, the main MCP tools are:

- `aivi.parse`
- `aivi.check`
- `aivi.fmt`
- `aivi.fmt.write` (requires `--allow-effects`)

These operate on explicit `target` arguments passed per tool call.

## GTK session tools

With `--ui`, the server also exposes GTK session management tools:

- `aivi.gtk.discover` — list candidate local GTK debug sockets
- `aivi.gtk.attach` — connect to an existing GTK session when you already know the socket path and token
- `aivi.gtk.launch` — start an AIVI GTK application under MCP inspection
- `aivi.gtk.hello` — verify that a session is alive and report high-level capabilities

`aivi.gtk.launch`, and all mutation-style GTK tools, require `--allow-effects`.

## GTK inspection tools

The first GTK inspection layer is read-only:

- `aivi.gtk.listWidgets`
- `aivi.gtk.inspectWidget`
- `aivi.gtk.dumpTree`

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
Window ids reported by `aivi.gtk.hello` can also be inspected by numeric `id`.

This is the preferred tool for targeted inspection because clients do not need to fetch and traverse the whole tree just to read one widget's props or dimensions.

### `dumpTree`

Returns the live widget tree, either for every root or for one `rootId`.

Each node includes its own props, dimensions, runtime state, bound signals, and children.

## GTK interaction tools

The GTK driver layer currently exposes:

- `aivi.gtk.click`
- `aivi.gtk.type`
- `aivi.gtk.select`
- `aivi.gtk.keyPress`

These operate on widgets previously discovered via `listWidgets`, `inspectWidget`, or `dumpTree`.

### Targeting rules

Widget targeting follows this order:

1. named widget (`name`)
2. numeric widget id (`id`)

If both are supplied, they must refer to the same live widget or the request fails.
`aivi.gtk.keyPress` may omit both `name` and `id`; in that case it targets the sole window when exactly one window is present, otherwise the request fails and the client must choose an explicit target.

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

```text
1. start `aivi mcp serve --ui --allow-effects`
2. call `aivi.gtk.launch` with a target `.aivi` app
3. call `aivi.gtk.listWidgets`
4. call `aivi.gtk.inspectWidget` for a named button or entry
5. call `aivi.gtk.click`, `aivi.gtk.type`, or `aivi.gtk.select`
6. re-read the widget or tree snapshot to observe the updated state
```
