# `aivi.ui.ServerHtml`
## Server-Driven HTML · Typed Events · Route-Based Serving

<!-- quick-info: {"kind":"module","name":"aivi.ui.ServerHtml"} -->
`aivi.ui.ServerHtml` provides server-driven UI rendering for AIVI apps. The server
renders `VNode msg` trees to HTML, the browser forwards delegated events and platform
signals over WebSocket, and the server sends DOM patch operations back.

<!-- /quick-info -->
<div class="import-badge">use aivi.ui.ServerHtml</div>

`aivi.ui.ServerHtml` is the recommended v0.1 backend for interactive browser UIs.

## Architecture

```text
Browser                                  Server
┌────────────────────────────┐          ┌────────────────────────────────────┐
│ HTTP GET /counter          ├─────────>│ serveHttp(route app)               │
│ (HTML + boot blob + JS)    │<─────────┤ renders initial VDOM               │
│                            │          │ stores view state by viewId         │
│ WS connect /counter/ws     ├─────────>│ serveWs(route app)                 │
│ hello(viewId,url,online)   │          │ validates viewId                   │
│ event/platform/effectResult├─────────>│ update + diff + effect handling    │
│ patch/effectReq            │<─────────┤ patch/effectReq/error              │
└────────────────────────────┘          └────────────────────────────────────┘
```

## Public API

```aivi
ViewId = Text

UrlInfo = { url: Text, path: Text, query: Text, hash: Text }
InitContext = { viewId: ViewId, url: UrlInfo, online: Bool }

PlatformEvent =
  PopState UrlInfo
  | HashChange { old: Text, new: Text, hash: Text, url: UrlInfo }
  | Visibility { visibilityState: Text }
  | WindowFocus { focused: Bool }
  | Online { online: Bool }
  | Intersection { sid: Int, entries: List { tid: Int, isIntersecting: Bool, ratio: Float } }

ClipboardError = { name: Text }

Effect msg =
  ClipboardReadText (Result ClipboardError Text -> msg)
  | ClipboardWriteText Text (Result ClipboardError Unit -> msg)
  | SubscribeIntersection
      { sid: Int, rootMargin: Text, threshold: List Float, targets: List { tid: Int, nodeId: Text } }
  | UnsubscribeIntersection Int

ServerHtmlApp model msg =
  { init: InitContext -> model
  , update: msg -> model -> (model, List (Effect msg))
  , view: model -> VNode msg
  , onPlatform: PlatformEvent -> Option msg
  }

Route model msg =
  { path: Text
  , app: ServerHtmlApp model msg
  }

serveHttp : ServerHtmlApp model msg -> Request -> Response
serveWs : ServerHtmlApp model msg -> WebSocket -> Effect WsError Unit
serve : ServerConfig -> List (Route model msg) -> Resource HttpError Server
```

## Example 1: Counter app with route-based server bootstrap

```aivi
use aivi
use aivi.ui
use aivi.net.httpServer
use aivi.ui.ServerHtml

Model = { count: Int }
Msg = Inc | Dec

view = model =>
  ~<html>
    <main>
      <button onClick={ Dec }>-</button>
      <span>{ model.count }</span>
      <button onClick={ Inc }>+</button>
    </main>
  </html>

update = msg model => msg match
  | Inc => (model <| { count: _ + 1 }, [])
  | Dec => (model <| { count: _ - 1 }, [])

counterApp : ServerHtmlApp Model Msg
counterApp =
  { init: _ => { count: 0 }
  , update: update
  , view: view
  , onPlatform: _ => None
  }

routes =
  [ { path: "/counter", app: counterApp } ]

main = resource {
  server <- serve { host: "127.0.0.1", port: 8080 } routes
  pure server
}
```

Notes:
- HTTP path is normalized (`/counter/` works).
- WebSocket path is derived as `/<route>/ws` (`/counter/ws` here).
- Unknown paths return `404`.

## Example 2: Platform + clipboard + intersection effects

```aivi
Model = { status: Text, clipboard: Text, heroVisible: Bool }

Msg =
  WentOffline
  | CameOnline
  | ReadClipboard
  | ClipboardResult (Result ClipboardError Text)
  | StartHeroWatch
  | HeroIntersected { sid: Int, entries: List { tid: Int, isIntersecting: Bool, ratio: Float } }

onPlatform = evt => evt match
  | Online { online } => if online then Some CameOnline else Some WentOffline
  | Intersection payload => Some (HeroIntersected payload)
  | _ => None

update = msg model => msg match
  | WentOffline => (model <| { status: "offline" }, [])
  | CameOnline => (model <| { status: "online" }, [])
  | ReadClipboard => (model, [ClipboardReadText ClipboardResult])
  | ClipboardResult (Ok text) => (model <| { clipboard: text }, [])
  | ClipboardResult (Err _) => (model, [])
  | StartHeroWatch =>
      (model,
        [SubscribeIntersection
          { sid: 1
          , rootMargin: "0px"
          , threshold: [0.0, 1.0]
          , targets: [{ tid: 1, nodeId: "hero" }]
          }
        ]
      )
  | HeroIntersected { entries } =>
      entries match
        | [{ isIntersecting: True, ..._ }, ..._] => (model <| { heroVisible: True }, [])
        | _ => (model, [])
```

## DOM event handlers and payloads

`aivi.ui.ServerHtml` supports delegated handlers encoded as `data-aivi-hid-*` attributes.

Supported event kinds on the wire:
- `click`, `input`
- `keydown`, `keyup`
- `pointerdown`, `pointerup`, `pointermove`
- `focus`, `blur`
- `transitionend`, `animationend`

`aivi.ui` attributes used with ServerHtml:
- `onClick` / `onClickE`
- `onInput` / `onInputE`
- `onKeyDown`, `onKeyUp`
- `onPointerDown`, `onPointerUp`, `onPointerMove`
- `onFocus`, `onBlur`
- `onTransitionEnd`, `onAnimationEnd`

## Runtime behavior details

- Each rendered element gets a stable `data-aivi-node` id.
- The client applies patch ops (`replace`, `setText`, `setAttr`, `removeAttr`) by node id.
- `serveHttp` allocates a fresh `viewId` and embeds it in the boot script.
- `serveWs` expects `hello` first; unknown `viewId` is rejected.
- `onPlatform` is optional via `Option msg`; return `None` to ignore a platform event.

## Wire protocol (JSON)

### Client → Server (`"t"` discriminator)

`hello`
```json
{ "t": "hello", "viewId": "<uuid>", "url": "https://example.com/counter", "online": true }
```

`event`
```json
{ "t": "event", "viewId": "<uuid>", "hid": 42, "kind": "click", "p": { "button": 0, "alt": false, "ctrl": false, "shift": false, "meta": false } }
```

`platform`
```json
{ "t": "platform", "viewId": "<uuid>", "kind": "visibility", "p": { "visibilityState": "hidden" } }
```

`effectResult`
```json
{ "t": "effectResult", "viewId": "<uuid>", "rid": 9001, "kind": "clipboard.readText", "ok": true, "p": { "text": "hello" } }
```

### Server → Client (`"t"` discriminator)

`patch`
```json
{ "t": "patch", "ops": "[{\"op\":\"setText\",\"id\":\"3\",\"text\":\"42\"}]" }
```

`subscribeIntersect`
```json
{ "t": "subscribeIntersect", "sid": 1, "options": { "rootMargin": "0px", "threshold": [0, 1] }, "targets": [{ "tid": 1, "nodeId": "hero" }] }
```

`effectReq`
```json
{ "t": "effectReq", "rid": 9001, "op": { "kind": "clipboard.writeText", "text": "copied" } }
```

`error`
```json
{ "t": "error", "code": "PAYLOAD", "detail": "invalid event payload" }
```

Error codes: `PROTO`, `DECODE`, `HID`, `PAYLOAD`, `PLATFORM`, `RID`.

## Client implementation and embedding

The browser client is built from `ui-client/` into one IIFE bundle
(`ui-client/dist/aivi-server-html-client.js`) and synced into runtime crates.

```bash
cd ui-client
pnpm install
pnpm build
node ./scripts/sync-to-rust.mjs
```

The sync copies the bundle to:
- `crates/aivi/src/runtime/builtins/ui/server_html_client.js`
- `crates/aivi_native_runtime/src/builtins/ui/server_html_client.js`

## v0.1 limits

- View state is socket-scoped; no reconnect state resume.
- Structural changes may emit subtree `replace` operations.
- Keyed reorders are represented as `replace` rather than a dedicated move op.
