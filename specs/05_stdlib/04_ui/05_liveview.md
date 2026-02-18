# Server-Driven UI (Concept)

> **Retired.** The `aivi.ui.live` concept has been fully absorbed into
> [`aivi.ui.ServerHtml`](06_server_html.md).
> See that page for the complete API, wire protocol, typed event payloads,
> browser client, and usage examples.

## Summary

Server-driven UI in AIVI v0.1 is provided exclusively by `aivi.ui.ServerHtml`:

- The server renders initial HTML from `VNode msg`.
- The browser forwards delegated DOM and platform events over WebSocket.
- The server updates the model, diffs the VDOM, and streams patch ops.

## Example

```aivi
use aivi.ui
use aivi.ui.layout
use aivi.net.httpServer
use aivi.ui.ServerHtml

Model = { count: Int }
Msg = Inc | Dec

view = model =>
  ~<html>
    <div style={ { width: 240px } }>
      <button onClick={ Dec }>-</button>
      <span>{ model.count }</span>
      <button onClick={ Inc }>+</button>
    </div>
  </html>

update = msg model => msg ?
  | Inc => (model <| { count: _ + 1 }, [])
  | Dec => (model <| { count: _ - 1 }, [])

app : App Model Msg
app =
  { init: _ => { count: 0 }
    update: update
    view: view
    onPlatform: _ => None
  }
```

## Protocol

See [ServerHtml § WebSocket Protocol](06_server_html.md#websocket-protocol-json)
for the full wire format. Key points:

- Stable node ids: `data-aivi-node="<id>"` (depth-first index).
- Handler ids: `data-aivi-hid-click="42"` etc.
- Patch ops: `replace`, `setText`, `setAttr`, `removeAttr`.
- Client→Server: `hello`, `event`, `platform`, `effectResult`.
- Server→Client: `patch`, `subscribeIntersect`, `unsubscribeIntersect`, `effectReq`, `error`.

## Limitations (v0.1)

- Diffing is conservative: structural changes may emit subtree `replace`.
- Keyed reorders are represented as `replace` rather than a "move" op.
- Reconnection creates a new view (no state resume).
