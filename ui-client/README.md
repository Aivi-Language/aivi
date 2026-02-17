# AIVI ServerHtml micro client

This folder contains the browser micro-client for `aivi.ui.ServerHtml`:

- applies DOM patch ops by `data-aivi-node`
- routes delegated DOM/platform events to the server
- handles a small set of browser effects (clipboard, IntersectionObserver)

Build (from repo root):

```bash
cd ui-client
npm install
npm run build
node ./scripts/sync-to-rust.mjs
```

The sync script copies the built bundle into:

- `crates/aivi/src/runtime/builtins/ui/server_html_client.js`
- `crates/aivi_native_runtime/src/builtins/ui/server_html_client.js`

