# `aivi.ui.gtk4`
## GTK & libadwaita Runtime Reference

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the native desktop UI system for AIVI. It mounts a live GTK tree, binds widget props and structure directly to `Signal` values, routes callbacks to runtime functions or `EventHandle` values, and updates only the mounted parts of the UI that depend on changed reactive data.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

This page is the stable entry point for the GTK module. If you want the guided story, start with [Mental Model](./gtk4/mental_model.md) and follow the pages in order.

## Writing Native Apps guide

1. [Mental Model](./gtk4/mental_model.md)
2. [First Window](./gtk4/first_window.md)
3. [State & Patches](./gtk4/state_patches.md)
4. [GTK Sugar](./gtk4/gtk_sugar.md)
5. [Callbacks](./gtk4/callbacks.md)
6. [Events](./gtk4/events.md)
7. [Structure](./gtk4/structure.md)
8. [Lifecycle](./gtk4/lifecycle.md)
9. [MCP Debugging](./gtk4/mcp_debugging.md)

## Core surface

The everyday surface is:

- state: `signal`, `get`, `peek`, `->>`, `<<-`, `combineAll`
- view: `~<gtk>...</gtk>`, shorthand widget tags, helper tags, `<show>`, `<each>`
- input: `onClick`, `onInput`, `onToggle`, `onSelect`, `onKeyPress`, and raw `<signal ... />`
- effectful actions: `do Event { ... }`
- app startup: `runGtkApp`
- lower-level escape hatches: `mountAppWindow`, `buildFromNode`, `buildWithIds`, `reconcileNode`, `signalStream`, `signalPoll`, `signalEmit`, `menuModelNew`, `menuModelAppendItem`, `menuButtonSetMenuModel`, `osOpenUri`, `gtkSetInterval`

## Lower-level helpers

| API | Use it when... |
| --- | --- |
| `mountAppWindow` | you need the mounted `WindowId` or several live roots |
| `buildFromNode` | you want to build a subtree from a GTK node |
| `buildWithIds` | you want the subtree plus a `Map Text WidgetId` for named widgets |
| `reconcileNode` | you are hosting or replacing a tree from lower-level code |
| `signalStream` | you want the raw GTK event stream |
| `signalPoll` | you want one queued `GtkSignalEvent` |
| `signalEmit` | you want to inject a synthetic event in tests or tooling |
| `widgetById` | you want to look up a named widget programmatically |
| `widgetSetCss` / `appSetCss` | you want imperative CSS injection |
| `drawAreaQueueDraw` | you want to queue redraw for a custom drawing surface |
| `menuModelNew` / `menuModelAppendItem` / `menuButtonSetMenuModel` | you are wiring programmatic GMenu infrastructure around a declarative `GtkMenuButton` |
| `osOpenUri` | you want to hand a URI to the desktop from a callback or helper |
| `gtkSetInterval` | you want a repeating low-level `GtkTick` feed for custom integrations |

The imperative escape-hatch surface is intentionally small. Public GTK helpers in this bucket are limited to menu plumbing, desktop URI handoff, and low-level tick feeds.

Imperative dialog construction (`dialog*`, `adwDialogPresent`, `signalBindDialogPresent`), tray/mail helpers, D-Bus server startup, badge counters, and theme probes are intentionally **not** part of the public GTK contract. Use mounted dialog roots with `open={...}` / `onClosed={...}` and ordinary callbacks instead.

For the tutorial flow around those helpers, read [Lifecycle](./gtk4/lifecycle.md). For live inspection and automation, read [MCP Debugging](./gtk4/mcp_debugging.md).

## Coverage and boundaries

The declarative GTK surface supports the full signal-first story plus a broad GTK4/libadwaita widget set.

The public contract is:

- any indexed `Gtk*` or `Adw*` class that is valid in the surrounding GTK API may be instantiated from `~<gtk>`, `buildFromNode`, or `buildWithIds`,
- writable scalar properties use existing tuned setters where needed and otherwise fall back to metadata-driven property application,
- object-valued properties can be expressed with nested `<property name="..."> <Gtk.../> </property>` nodes,
- common single-child containers can attach through common pointer properties such as `child` and `content`,
- container-specific `<child type="...">` attachment remains a thin handwritten layer for surfaces that need specialized add/remove calls,
- typed callback payloads stay curated for the common sugar surface, while broader indexed signals may still surface through `GtkUnknownSignal`.

That means the declarative tree is not limited to a tiny hand-wrapped widget set. In addition to `GtkBox`, `GtkButton`, and `GtkEntry`, the current surface also covers broader families such as `GtkListView`, `GtkColumnView`, `GtkGridView`, `GtkPopover`, `GtkPopoverMenu`, `GtkPopoverMenuBar`, `GtkVideo`, `GtkMediaControls`, `GtkStringList`, `GtkAdjustment`, `GtkSelectionModel` implementations, `GtkFilterListModel`, `GtkSortListModel`, `GtkSignalListItemFactory`, `GtkBuilderListItemFactory`, and layout-manager objects when they are connected through the right property.

Some GTK and libadwaita concerns still make more sense as dedicated imperative helpers than as ordinary tree nodes. The current public surface keeps only:

- application-level action/menu plumbing via `menuModelNew`, `menuModelAppendItem`, and `menuButtonSetMenuModel`,
- desktop URI handoff via `osOpenUri`,
- raw main-loop tick feeds via `gtkSetInterval`.

## Diagnostics

| Code | Condition |
| --- | --- |
| `E1612` | invalid `props` shape (must be a record literal the sigil can inspect) |
| `E1613` | unsupported `props` field value in a position that requires static inspection |
| `E1614` | invalid signal binding (`onClick`, `onInput`, `onActivate`, `onKeyPress`, `onToggle`, `onSelect`, `onClosed`, `onValueChanged`, `onFocusIn`, `onFocusOut`, `onShowSidebarChanged`, and `<signal ... on={...}>` require either a function callback with the documented payload shape or an `EventHandle`) |
| `E1615` | invalid `<each>` usage (requires `items={...}`, `as={...}`, and exactly one child template node; keyed iteration is the recommended contract) |
| `E1616` | bare `<child>` without a `type` attribute |
| `E1617` | invalid GTK function-call tag usage (function-call sugar must use positional arguments on a self-closing tag and cannot mix with attributes) |

Runtime binding failures report the widget id/class, optional `id="..."` name, the failing binding, and the known supported signals for that class.

## Where to go next

- [Mental Model](./gtk4/mental_model.md) to start the full native-app guide
- [Signals](./reactive_signals.md) for the day-to-day reactive API
- [Reactive Dataflow](./reactive_dataflow.md) for batching, invalidation, and lifecycle cleanup
- [Forms](./forms.md) for typed field state and validation patterns on top of GTK callbacks
- [MCP Server](../../tools/mcp.md)
