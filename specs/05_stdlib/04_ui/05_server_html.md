# `aivi.ui.ServerHtml`
## Server-Driven HTML · DOM Patching · Typed Events

<!-- quick-info: {"kind":"module","name":"aivi.ui.ServerHtml"} -->
`aivi.ui.ServerHtml` is a server-driven UI runtime. The server renders HTML from
a `VNode msg` tree, the browser forwards delegated DOM and platform events over a
WebSocket as typed JSON messages, and the server diffs the VDOM to stream patch ops
back. No client-side VDOM is needed — patches target stable `data-aivi-node` ids.

<!-- /quick-info -->

`aivi.ui.ServerHtml` is the recommended v0.1 backend bootstrap for server-driven UIs.

## Architecture Overview

```
 Browser                        Server
┌──────────────────┐     ┌───────────────────────┐
│ Initial HTML GET ├────>│ serveHttp             │
│                  │<────┤  app.init → VNode     │
│                  │     │  renderHtml → HTML    │
│ WebSocket        │     │                       │
│  hello ──────────┼────>│ serveWs               │
│  event ──────────┼────>│  decodeEvent → msg    │
│  platform ───────┼────>│  app.update → model'  │
│  effectResult ───┼────>│  diff old new → ops   │
│                  │<────┤  patch (ops) ─────────│
│                  │<────┤  effectReq ───────────│
└──────────────────┘     └───────────────────────┘
```

## Quick Start Example

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

update = msg model => msg match
  | Inc => (model <| { count: _ + 1 }, [])
  | Dec => (model <| { count: _ - 1 }, [])

app : App Model Msg
app =
  { init: _ => { count: 0 }
    update: update
    view: view
    onPlatform: _ => None
  }

main = do Effect {
  req <- receiveHttpRequest   // pseudo — depends on httpServer.listen
  resp <- serveHttp app req
  pure resp
}
```

## Public API

### Types

```aivi
ViewId         = Text        -- opaque; one per WebSocket connection
HandlerId      = Int
SubscriptionId = Int
RequestId      = Int

UrlInfo = { href: Text, path: Text, query: Text, hash: Text }

InitContext = { viewId: ViewId, url: UrlInfo, online: Bool }
```

### App record

The `App` record is the user-facing entry point. It defines how to initialise a
model, update it when messages arrive, render it to a virtual DOM, and optionally
react to browser platform events.

```aivi
App model msg = {
  init       : InitContext -> model,
  update     : msg -> model -> (model, List (AppEffect model msg)),
  view       : model -> VNode msg,
  onPlatform : PlatformEvent -> Option msg
}
```

### AppEffect

Effects that an `update` function can return alongside the new model:

```aivi
AppEffect model msg =
  | ReadClipboard  (Result ClipboardError Text -> msg)
  | WriteClipboard Text (Result ClipboardError Unit -> msg)
  | SubIntersect   SubscriptionId IntersectionOptions (List IntersectionTarget)
  | UnsubIntersect SubscriptionId
```

#### Clipboard example

```aivi
Msg = Paste (Result ClipboardError Text) | RequestPaste

update = msg model => msg match
  | RequestPaste   => (model, [ReadClipboard Paste])
  | Paste (Ok txt) => (model <| { clipboard: txt }, [])
  | Paste (Err _)  => (model, [])
```

#### IntersectionObserver example

```aivi
Msg = BecameVisible { sid: SubscriptionId, entries: List IntersectionEntry }
    | WatchSection

update = msg model => msg match
  | WatchSection =>
    (model, [SubIntersect 1
               { rootMargin: "0px", threshold: [0.0, 1.0] }
               [{ tid: 1, nodeId: "section-hero" }]])
  | BecameVisible _ => (model <| { heroVisible: True }, [])
```

### ClipboardError

```aivi
ClipboardError =
  | PermissionDenied
  | Unavailable
  | Other Text
```

### serveHttp

```aivi
serveHttp : App model msg -> Request -> Effect HttpError Response
```

Handles an incoming HTTP request by:

1. Generating a fresh `ViewId` (UUID).
2. Parsing `UrlInfo` from the request path.
3. Calling `app.init` to create the initial model.
4. Rendering the model's view to HTML with handler-id attributes.
5. Embedding the boot blob (`viewId`, `wsUrl`) and the compiled JS client.
6. Returning a `200 text/html` response.

### serveWs

```aivi
serveWs : App model msg -> WebSocket -> Effect WsError Unit
```

Handles a WebSocket connection by:

1. Waiting for a `Hello` message from the client.
2. Creating a fresh `ViewId` and initialising the model.
3. Sending an initial `Patch` with the full rendered HTML.
4. Entering a tail-recursive message loop that decodes incoming messages,
   dispatches them through `app.update`, diffs the VDOM, and sends patches.

## Event Payloads

All DOM events are forwarded with typed payloads. Field names match exactly
between AIVI types, the wire protocol, and the TypeScript client.

```aivi
ClickPayload = { button: Int, alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }

InputPayload = { value: Text }

KeyPayload = { key: Text, code: Text, alt: Bool, ctrl: Bool,
               shift: Bool, meta: Bool, repeat: Bool, isComposing: Bool }

PointerPayload = { pointerId: Int, pointerType: Text, button: Int, buttons: Int,
                   clientX: Int, clientY: Int,
                   alt: Bool, ctrl: Bool, shift: Bool, meta: Bool }

EventPayload =
  | ClickEvt   ClickPayload
  | InputEvt   InputPayload
  | KeyDownEvt KeyPayload
  | KeyUpEvt   KeyPayload
  | PtrDownEvt PointerPayload
  | PtrUpEvt   PointerPayload
  | PtrMoveEvt PointerPayload
```

Event kind strings on the wire: `"click"`, `"input"`, `"keydown"`, `"keyup"`,
`"pointerdown"`, `"pointerup"`, `"pointermove"`.

## Platform Events

Browser-level signals forwarded as typed ADTs:

```aivi
PlatformEvent =
  | PopState   UrlInfo
  | HashChange { url: UrlInfo, oldURL: Text, newURL: Text, hash: Text }
  | Visibility { state: Text }
  | WindowFocus { focused: Bool }
  | Online      { online: Bool }
  | Intersection { sid: SubscriptionId, entries: List IntersectionEntry }
```

Platform kind strings: `"popstate"`, `"hashchange"`, `"visibility"`, `"focus"`,
`"online"`, `"intersection"`.

### Platform event example

```aivi
onPlatform = evt => evt match
  | PopState url       => Some (NavigateTo url.path)
  | Online { online }  => online match
    | True  => Some Reconnected
    | False => Some Disconnected
  | _                  => None
```

## DOM Identity

- Every rendered element receives a `data-aivi-node` attribute with a stable
  depth-first index. The same tree structure always produces the same ids.
- Event handlers are embedded as `data-aivi-hid-<kind>="<id>"` attributes
  (e.g. `data-aivi-hid-click="42"`).
- The client walks up from `event.target` to find the nearest matching
  `data-aivi-hid-<kind>` attribute.

## Handler Assignment

During rendering, the runtime walks the VDOM and replaces each event-handler
`Attr` with a `data-aivi-hid-<kind>` attribute, recording a mapping from
`HandlerId` to the handler function.

```aivi
assignHandlers
  : VNode msg
  -> HandlerId
  -> (VNode msg, Map HandlerId (EventPayload -> Option msg), HandlerId)
```

## WebSocket Protocol (JSON)

### Client → Server

All client messages use `"t"` as the discriminator field.

#### `hello`

```json
{ "t": "hello", "viewId": "<uuid>", "url": "https://example.com/app#x", "online": true }
```

Sent once on connection open. `viewId` comes from the boot blob embedded in the
initial HTML page.

#### `event`

```json
{ "t": "event", "viewId": "<uuid>", "hid": 42, "kind": "click",
  "p": { "button": 0, "alt": false, "ctrl": false, "shift": false, "meta": false } }
```

#### `platform`

```json
{ "t": "platform", "viewId": "<uuid>", "kind": "popstate",
  "p": { "href": "https://x.com/a", "path": "/a", "query": "", "hash": "" } }
```

#### `effectResult`

```json
{ "t": "effectResult", "viewId": "<uuid>", "rid": 9001,
  "kind": "clipboard.readText", "ok": true, "p": { "text": "pasted content" } }
```

On failure:

```json
{ "t": "effectResult", "viewId": "<uuid>", "rid": 9001,
  "kind": "clipboard.readText", "ok": false, "error": "NotAllowedError" }
```

### Server → Client

#### `patch`

```json
{ "t": "patch", "ops": "[{\"op\":\"setText\",\"id\":\"3\",\"text\":\"42\"}]" }
```

The `ops` field is a JSON-encoded string containing an array of patch operations.
Supported ops: `replace`, `setText`, `setAttr`, `removeAttr`.

#### `subscribeIntersect`

```json
{ "t": "subscribeIntersect", "sid": 1,
  "options": { "rootMargin": "0px", "threshold": [0, 1] },
  "targets": [{ "tid": 1, "nodeId": "42" }] }
```

#### `unsubscribeIntersect`

```json
{ "t": "unsubscribeIntersect", "sid": 1 }
```

#### `effectReq`

```json
{ "t": "effectReq", "rid": 9001,
  "op": { "kind": "clipboard.writeText", "text": "hello" } }
```

#### `error`

```json
{ "t": "error", "code": "PAYLOAD", "detail": "invalid event payload" }
```

Error codes: `"PROTO"`, `"DECODE"`, `"HID"`, `"PAYLOAD"`, `"PLATFORM"`, `"RID"`.

## Module Layout

The implementation is split across helper modules:

| Module | Responsibility |
| :--- | :--- |
| `aivi.ui.serverHtml.Protocol` | Wire types, JSON encode/decode for all messages |
| `aivi.ui.serverHtml.Runtime` | ViewState, handler assignment, update loop helpers |
| `aivi.ui.serverHtml.ClientAsset` | `@static` embed of the compiled browser client JS |
| `aivi.ui.ServerHtml` | Public API: `App`, `serveHttp`, `serveWs` |

## Browser Client

The browser client lives in `ui-client/` as TypeScript compiled with Vite into
a single IIFE bundle (`ui-client/dist/aivi-server-html-client.js`). It is
embedded via `@static` at compile time — no raw JS strings in AIVI source.

The client:

- Reads the boot blob from `<script id="aivi-server-html-boot">` on page load.
- Opens a WebSocket and sends `hello`.
- Applies DOM patches by `data-aivi-node` id (cache + `querySelector` fallback).
- Delegates ONE listener per event kind to `document` and walks `composedPath()`
  to find the nearest `data-aivi-hid-<kind>` attribute.
- Manages `IntersectionObserver` subscriptions with per-sid batching
  (flushed once per `requestAnimationFrame`).
- Executes clipboard effects (`navigator.clipboard`) and returns results.
- Reconnects with exponential backoff (1 s → 30 s, unlimited retries).

### Client source layout (`ui-client/src/`)

| File | Role |
| :--- | :--- |
| `types.ts` | TypeScript interfaces mirroring Protocol.aivi wire types |
| `patch.ts` | DOM patch application with node cache |
| `events.ts` | Delegated listeners + payload extractors |
| `intersection.ts` | IntersectionObserver manager |
| `clipboard.ts` | Clipboard effect executor |
| `ws.ts` | WebSocket lifecycle + reconnect |
| `main.ts` | Entry point (IIFE) + platform listeners |

## Security Notes (v0.1)

- Treat all inbound messages as untrusted.
- `ViewId` must be unguessable (UUID).
- Production deployments should bind `ViewId` to an additional secret/token
  (out of scope for v0.1).

## Limitations (v0.1)

- `ViewId` is per-socket only; no cross-connection session persistence.
- Diffing is conservative: when structure or keyed child segments change,
  the runtime may emit a subtree `replace`.
- Keyed reorders are represented as `replace` rather than a dedicated "move" op.
- Reconnection policy v1: every reconnect `hello` creates a new view (no state resume).
