# LiveView-Style Server-Driven UI

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
LiveView-style server-driven UI in v0.1 is provided by `aivi.ui.ServerHtml`:

- server renders initial HTML from `VNode msg`
- browser forwards delegated DOM/platform events over WebSocket
- server updates the model and streams DOM patch ops

<!-- /quick-info -->

Note: older docs referenced a minimal `aivi.ui.live`. That API has been removed; use `aivi.ui.ServerHtml.serve` instead.
<<< ../../snippets/from_md/05_stdlib/04_ui/05_liveview/block_01.aivi{aivi}

## API Shape

```aivi
serve
  : aivi.net.http_server.ServerConfig
  -> List (aivi.ui.ServerHtml.Route model msg)
  -> Resource aivi.net.http_server.HttpError aivi.net.http_server.Server
```

## Protocol (Browser <-> Server)

### Stable node ids

The HTML renderer attaches a stable node id to every rendered node:

- `data-aivi-node="root/..."` (string ids derived from tree position and keys)

### Patch messages (server -> browser)

The server sends JSON messages shaped like:

```json
{"t":"patch","ops":[ ... ]}
```

Where each op is one of:

- `{"op":"replace","id":"...","html":"<div ...>...</div>"}`
- `{"op":"setText","id":"...","text":"..."}`
- `{"op":"setAttr","id":"...","name":"class","value":"..."}`
- `{"op":"removeAttr","id":"...","name":"class"}`

### Event messages (browser -> server)

The embedded client delegates events and sends JSON:

- click: `{"t":"click","id":123}`
- input: `{"t":"input","id":123,"value":"..."}` where `value` is taken from the event target

The event `id` identifies the handler attached by the server for that node.

## Limitations (v0.1)

- Diffing is conservative: when structure or keyed child segments change, the runtime may emit a subtree `replace`.
- Keyed reorders are represented as `replace` rather than a dedicated "move" op.
