# `aivi.ui.ServerHtml`
## Server-Driven HTML · DOM Patching · Typed Events

<!-- quick-info: {"kind":"module","name":"aivi.ui.ServerHtml"} -->
`aivi.ui.ServerHtml` is a server-driven UI runtime. The server renders HTML from
a `VNode msg` tree, the browser forwards delegated DOM and platform events over a
WebSocket as typed JSON messages, and the server diffs the VDOM to stream patch ops
back. No client-side VDOM is needed   patches target stable `data-aivi-node` ids.

<!-- /quick-info -->
<div class="import-badge">use aivi.ui.ServerHtml</div>


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

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_01.aivi{aivi}


## Public API

### Types

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_02.aivi{aivi}


### App record

The `App` record is the user-facing entry point. It defines how to initialise a
model, update it when messages arrive, render it to a virtual DOM, and optionally
react to browser platform events.

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_03.aivi{aivi}


### AppEffect

Effects that an `update` function can return alongside the new model:

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_04.aivi{aivi}


#### Clipboard example

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_05.aivi{aivi}


#### IntersectionObserver example

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_06.aivi{aivi}


### ClipboardError

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_07.aivi{aivi}


### serveHttp

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_08.aivi{aivi}


Handles an incoming HTTP request by:

1. Generating a fresh `ViewId` (UUID).
2. Parsing `UrlInfo` from the request path.
3. Calling `app.init` to create the initial model.
4. Rendering the model's view to HTML with handler-id attributes.
5. Embedding the boot blob (`viewId`, `wsUrl`) and the compiled JS client.
6. Returning a `200 text/html` response.

### serveWs

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_09.aivi{aivi}


Handles a WebSocket connection by:

1. Waiting for a `Hello` message from the client.
2. Creating a fresh `ViewId` and initialising the model.
3. Sending an initial `Patch` with the full rendered HTML.
4. Entering a tail-recursive message loop that decodes incoming messages,
   dispatches them through `app.update`, diffs the VDOM, and sends patches.

## Event Payloads

All DOM events are forwarded with typed payloads. Field names match exactly
between AIVI types, the wire protocol, and the TypeScript client.

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_10.aivi{aivi}


Event kind strings on the wire: `"click"`, `"input"`, `"keydown"`, `"keyup"`,
`"pointerdown"`, `"pointerup"`, `"pointermove"`.

## Platform Events

Browser-level signals forwarded as typed ADTs:

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_11.aivi{aivi}


Platform kind strings: `"popstate"`, `"hashchange"`, `"visibility"`, `"focus"`,
`"online"`, `"intersection"`.

### Platform event example

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_12.aivi{aivi}


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

<<< ../../snippets/from_md/05_stdlib/04_ui/05_server_html/block_13.aivi{aivi}


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
embedded via `@static` at compile time   no raw JS strings in AIVI source.

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
