# LiveView-Style Server-Driven UI

<!-- quick-info: {"kind":"module","name":"aivi.ui"} -->
`aivi.ui.live` starts an HTTP server that:

- serves an initial HTML page rendered from `view : model -> VNode msg`, and
- accepts browser events over a WebSocket and responds with VDOM diffs as patches.

<!-- /quick-info -->

Note: `aivi.ui.live` is the minimal v0.1 LiveView runtime. For typed event payloads, platform signals, IntersectionObserver subscriptions, and clipboard effects, use `aivi.ui.ServerHtml`.
<<< ../../snippets/from_md/05_stdlib/04_ui/05_liveview/block_01.aivi{aivi}

## API Shape

```aivi
live
  : LiveConfig
  -> model
  -> (model -> VNode msg)
  -> (msg -> model -> model)
  -> Effect LiveError Server
```

`LiveConfig` is a record:

- `address : Text` (e.g. `"127.0.0.1:3000"`)
- `path : Text` (e.g. `"/"`)
- `title : Text` (HTML `<title>`)

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
