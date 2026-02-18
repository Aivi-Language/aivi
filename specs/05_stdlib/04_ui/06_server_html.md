# `aivi.ui.ServerHtml` (Server-Driven HTML + DOM Patching + Typed Events)

<!-- quick-info: {"kind":"module","name":"aivi.ui.ServerHtml"} -->
`aivi.ui.ServerHtml` is a LiveView-style server-driven UI runtime:

- Server renders initial HTML from `VNode msg`.
- Browser forwards delegated DOM/platform events over WebSocket as JSON.
- Server updates the model, re-renders VDOM, diffs, and streams patch ops.
- Browser applies patch ops by stable `data-aivi-node` ids (no client VDOM).

<!-- /quick-info -->

`aivi.ui.ServerHtml` is the recommended v0.1 backend bootstrap for server-driven UIs.
It includes a higher-level server helper (`serve`) that:

- serves the initial page for each route,
- hosts the matching WebSocket endpoints, and
- keeps typed event payloads end-to-end.

## Goals (v0.1)

- Stable node ids in HTML: `data-aivi-node="<id>"`.
- Patch ops: `replace`, `setText`, `setAttr`, `removeAttr`.
- Delegated DOM events with typed payloads:
  - `click`, `input`
  - `keydown`, `keyup`
  - `pointerdown`, `pointerup`, `pointermove`
  - `focus`, `blur`
- Platform signals as typed events:
  - navigation: `popstate`, `hashchange`
  - visibility: `visibilitychange`
  - window focus/blur
  - online/offline
  - IntersectionObserver (in-view) subscriptions
- Clipboard as a typed effect (request/response):
  - `clipboard.readText`
  - `clipboard.writeText`

## DOM Identity

- Every renderable node must include a stable `data-aivi-node` attribute.
- Server-to-client patch ops target `data-aivi-node` ids.
- For keyed lists, stable keys should preserve node ids across renders.

## Handler Ids

Element handlers are embedded as attributes:

- `data-aivi-hid-click="123"`
- `data-aivi-hid-input="124"`
- `data-aivi-hid-keydown="125"`
- ...

The client walks up from `event.target` to find the nearest matching handler id for that `kind`.

## WebSocket Protocol (JSON)

### Client -> Server

- `hello`

```json
{ "t":"hello", "viewId":"<uuid>", "url":"https://example/app#x", "online":true }
```

- `event`

```json
{ "t":"event", "viewId":"<uuid>", "hid":123, "kind":"click", "p":{...} }
```

- `platform`

```json
{ "t":"platform", "viewId":"<uuid>", "kind":"hashchange", "p":{ "url":"...", "hash":"#a" } }
```

- `effectResult` (clipboard)

```json
{ "t":"effectResult", "viewId":"<uuid>", "rid":9001, "kind":"clipboard.readText", "ok":true, "p":{ "text":"..." } }
```

On error:

```json
{ "t":"effectResult", "viewId":"<uuid>", "rid":9001, "kind":"clipboard.readText", "ok":false, "error":"NotAllowedError" }
```

### Server -> Client

- `patch`

```json
{ "t":"patch", "viewId":"<uuid>", "ops":[ ... ] }
```

- `subscribe` / `unsubscribe` (IntersectionObserver)

```json
{ "t":"subscribe", "viewId":"<uuid>", "kind":"intersection", "sid":77,
  "p": { "root": null, "rootMargin":"0px", "threshold":[0, 1] },
  "targets":[ { "tid":1, "nodeId":"n555" } ]
}
```

```json
{ "t":"unsubscribe", "viewId":"<uuid>", "kind":"intersection", "sid":77 }
```

- `effect` (clipboard)

```json
{ "t":"effect", "viewId":"<uuid>", "rid":9001, "kind":"clipboard.writeText", "p":{ "text":"hello" } }
```

- `error`

```json
{ "t":"error", "viewId":"<uuid>", "message":"...", "code":"..." }
```

## Security Notes (v0.1)

- Treat all inbound messages as untrusted.
- `ViewId` must be unguessable (UUID).
- Production deployments should bind `ViewId` to an additional secret/token (out of scope here).

## Server Bootstrap (Routing)

`aivi.ui.ServerHtml` can bootstrap an HTTP + WebSocket server via `aivi.net.httpServer.listen`.

### API Shape

```aivi
Route model msg =
  { path: Text
  , app: ServerHtmlApp model msg
  }

serve
  : aivi.net.httpServer.ServerConfig
  -> List (Route model msg)
  -> Resource aivi.net.httpServer.HttpError aivi.net.httpServer.Server
```

Routing is path-based:

- HTTP requests to `route.path` serve the initial HTML page.
- WebSocket upgrades at `wsPath(route.path)` serve the matching live session.

Where `wsPath("/") == "/ws"` and otherwise `wsPath("/x") == "/x/ws"` (with trailing slashes normalized).
