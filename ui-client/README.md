# AIVI ServerHtml Browser Client

This folder contains the browser micro-client for `aivi.ui.ServerHtml`.

## What it does

- Applies DOM patch ops by `data-aivi-node` attribute (node cache + `querySelector` fallback)
- Routes delegated DOM events (`click`, `input`, `keydown`, `keyup`, `pointerdown`, `pointerup`, `pointermove`, `transitionend`, `animationend`) to the server via WebSocket
- Forwards browser platform signals (`popstate`, `hashchange`, `visibilitychange`, `focus`/`blur`, `online`/`offline`) to the server
- Manages `IntersectionObserver` subscriptions with per-sid batched flushing
- Executes clipboard effects (`navigator.clipboard.readText` / `writeText`) and returns results
- Reconnects automatically with exponential backoff (1 s → 30 s, unlimited retries)

## Source structure (`src/`)

| File | Role |
| :--- | :--- |
| `types.ts` | TypeScript interfaces mirroring `aivi.ui.serverHtml.Protocol` wire types |
| `patch.ts` | DOM patch application with `nodeCache` |
| `events.ts` | Delegated listeners (one per kind on `document`) + payload extractors |
| `intersection.ts` | `IntersectionObserver` manager (subscribe/unsubscribe by `sid`) |
| `clipboard.ts` | Clipboard effect executor (read/write) |
| `ws.ts` | WebSocket lifecycle, reconnect, message dispatch |
| `main.ts` | Entry point (IIFE)   boots WS, installs event + platform listeners |

## Build

```bash
cd ui-client
pnpm install
pnpm build
```

The Vite build outputs a single IIFE bundle to `dist/aivi-server-html-client.js`.

## Sync to Rust

After building, copy the bundle into the Rust runtime crates:

```bash
node ./scripts/sync-to-rust.mjs
```

This copies the built bundle to:

- `crates/aivi/src/runtime/builtins/ui/server_html_client.js`
- `crates/aivi_native_runtime/src/builtins/ui/server_html_client.js`

## Wire protocol

The client communicates with the server using JSON over WebSocket.
See the [ServerHtml spec](../specs/stdlib/ui/06_server_html.md) for the
full protocol documentation.

### Client → Server messages (discriminator: `"t"`)

- `hello`   sent on connection open
- `event`   DOM event with handler id, kind, and typed payload
- `platform`   browser platform signal (popstate, visibility, etc.)
- `effectResult`   response to a server-initiated effect (clipboard)

### Server → Client messages (discriminator: `"t"`)

- `patch`   DOM patch ops (replace, setText, setAttr, removeAttr)
- `subscribeIntersect` / `unsubscribeIntersect`   IntersectionObserver management
- `effectReq`   clipboard read/write request
- `error`   server-side error notification
