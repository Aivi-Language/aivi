# `aivi.ui.gtk4`
## GTK & libadwaita Runtime for Native Apps

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the runtime module for native desktop apps built with GTK4 and libadwaita. In plain language, it is the layer that turns `GtkNode` trees and `GtkSignalEvent` values into a running desktop app.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

Start with [Native GTK & libadwaita Apps](./native_gtk_apps.md) for the high-level guide and [GTK App Architecture](./app_architecture.md) for the event-loop details. This page is the practical API reference.

## How UI effects fit into AIVI

GTK runtime operations are ordinary effects and resources in AIVI:

- widget and window construction/presentation → `ui.window`,
- `signalPoll`, `signalStream`, `signalEmit` → `ui.signal`,
- clipboard helpers → `ui.clipboard`,
- desktop notification helpers → `ui.notification`,
- `gtkApp` → the coarse-grained `ui` entry point.

These operations live in AIVI's ordinary effect system. They do not create a separate “special UI language”.

## Start here

Most single-window GTK apps only need this smaller subset:

- `gtkApp` — the standard event-loop host
- `GtkNode` and `GtkSignalEvent` — widget-tree and input-event types
- `~<gtk>...</gtk>` — the usual way to build the view tree
- timer helpers such as `commandAfter` or `subscriptionEvery`
- `reconcileNode` only indirectly, because `gtkApp` calls it for you

The rest of this page is still important reference material, but you do not need to memorize all of it before building a first app.

## Choosing the right entry point

| If you need to... | Start with... |
| --- | --- |
| build a normal single-window desktop app | `gtkApp` |
| describe a widget tree in the most readable way | `~<gtk>...</gtk>` with shorthand tags |
| map widget input into app messages | signal sugar plus `toMsg` or `toMsg: auto` |
| run a custom loop or experiment with manual hosting | `signalStream`, `buildFromNode`, `reconcileNode` |
| inspect the signal queue one event at a time | `signalPoll` |
| grab widget ids for direct low-level access | `buildWithIds` |

## Minimal `gtkApp` example

If you want one tiny anchor before the reference sections, start here:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_01.aivi{aivi}


## Selected public API surface

This condensed signature block is meant to help you orient yourself quickly. The module also exports additional lower-level setters, widget helpers, and compatibility functions that later sections call out when they matter.

<<< ../../snippets/from_md/stdlib/ui/gtk4/public_api.aivi{aivi}

## Native mapping table

Treat this table as a reference shelf, not a first-read tutorial. It highlights common lower-level functions that forward directly to GTK or libadwaita counterparts:

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
| `windowNew` | `gtk4.windowNew` |
| `windowSetTitle` | `gtk4.windowSetTitle` |
| `windowSetTitlebar` | `gtk4.windowSetTitlebar` |
| `windowSetChild` | `gtk4.windowSetChild` |
| `windowPresent` | `gtk4.windowPresent` |
| `windowClose` | `gtk4.windowClose` |
| `windowOnClose` | `gtk4.windowOnClose` |
| `windowSetHideOnClose` | `gtk4.windowSetHideOnClose` |
| `windowSetDecorated` | `gtk4.windowSetDecorated` |
| `appRun` | `gtk4.appRun` |
| `widgetShow` | `gtk4.widgetShow` |
| `widgetHide` | `gtk4.widgetHide` |
| `boxNew` | `gtk4.boxNew` |
| `boxAppend` | `gtk4.boxAppend` |
| `buttonNew` | `gtk4.buttonNew` |
| `buttonSetLabel` | `gtk4.buttonSetLabel` |
| `labelNew` | `gtk4.labelNew` |
| `labelSetText` | `gtk4.labelSetText` |
| `entryNew` | `gtk4.entryNew` |
| `entrySetText` | `gtk4.entrySetText` |
| `entryText` | `gtk4.entryText` |
| `scrollAreaNew` | `gtk4.scrollAreaNew` |
| `scrollAreaSetChild` | `gtk4.scrollAreaSetChild` |
| `dragSourceNew` | `gtk4.dragSourceNew` |
| `dragSourceSetText` | `gtk4.dragSourceSetText` |
| `dropTargetNew` | `gtk4.dropTargetNew` |
| `dropTargetLastText` | `gtk4.dropTargetLastText` |
| `menuModelNew` | `gtk4.menuModelNew` |
| `menuModelAppendItem` | `gtk4.menuModelAppendItem` |
| `menuButtonNew` | `gtk4.menuButtonNew` |
| `menuButtonSetMenuModel` | `gtk4.menuButtonSetMenuModel` |
| `dialogNew` | `gtk4.dialogNew` |
| `dialogSetTitle` | `gtk4.dialogSetTitle` |
| `dialogSetChild` | `gtk4.dialogSetChild` |
| `dialogPresent` | `gtk4.dialogPresent` |
| `dialogClose` | `gtk4.dialogClose` |
| `fileDialogNew` | `gtk4.fileDialogNew` |
| `fileDialogSelectFile` | `gtk4.fileDialogSelectFile` |
| `imageNewFromFile` | `gtk4.imageNewFromFile` |
| `imageSetFile` | `gtk4.imageSetFile` |
| `imageNewFromResource` | `gtk4.imageNewFromResource` |
| `imageSetResource` | `gtk4.imageSetResource` |
| `listStoreNew` | `gtk4.listStoreNew` |
| `listStoreAppendText` | `gtk4.listStoreAppendText` |
| `listStoreItems` | `gtk4.listStoreItems` |
| `listViewNew` | `gtk4.listViewNew` |
| `listViewSetModel` | `gtk4.listViewSetModel` |
| `treeViewNew` | `gtk4.treeViewNew` |
| `treeViewSetModel` | `gtk4.treeViewSetModel` |
| `gestureClickNew` | `gtk4.gestureClickNew` |
| `gestureClickLastButton` | `gtk4.gestureClickLastButton` |
| `widgetAddController` | `gtk4.widgetAddController` |
| `clipboardDefault` | `gtk4.clipboardDefault` |
| `clipboardSetText` | `gtk4.clipboardSetText` |
| `clipboardText` | `gtk4.clipboardText` |
| `actionNew` | `gtk4.actionNew` |
| `actionSetEnabled` | `gtk4.actionSetEnabled` |
| `appAddAction` | `gtk4.appAddAction` |
| `shortcutNew` | `gtk4.shortcutNew` |
| `widgetAddShortcut` | `gtk4.widgetAddShortcut` |
| `drawAreaNew` | `gtk4.drawAreaNew` |
| `drawAreaSetContentSize` | `gtk4.drawAreaSetContentSize` |
| `drawAreaQueueDraw` | `gtk4.drawAreaQueueDraw` |
| `widgetSetCss` | `gtk4.widgetSetCss` |
| `appSetCss` | `gtk4.appSetCss` |
| `notificationNew` | `gtk4.notificationNew` |
| `notificationSetBody` | `gtk4.notificationSetBody` |
| `appSendNotification` | `gtk4.appSendNotification` |
| `appWithdrawNotification` | `gtk4.appWithdrawNotification` |
| `layoutManagerNew` | `gtk4.layoutManagerNew` |
| `widgetSetLayoutManager` | `gtk4.widgetSetLayoutManager` |
| `buildFromNode` | `gtk4.buildFromNode` |
| `buildWithIds` | `gtk4.buildWithIds` |
| `reconcileNode` | `gtk4.reconcileNode` |
| `signalPoll` | `gtk4.signalPoll` |
| `signalStream` | `gtk4.signalStream` |
| `signalEmit` | `gtk4.signalEmit` |
| `osOpenUri` | `gtk4.osOpenUri` |
| `osShowInFileManager` | `gtk4.osShowInFileManager` |
| `osSetBadgeCount` | `gtk4.osSetBadgeCount` |
| `osThemePreference` | `gtk4.osThemePreference` |
| `gtkApp` | (AIVI-level combinator) |

## Example

Start here if you want to see the module used as a whole before drilling into the details:

<<< ../../snippets/from_md/stdlib/ui/gtk4/example.aivi{aivi}

## GTK XML sigil (`~<gtk>...</gtk>`)

The GTK sigil is the most approachable way to describe widget trees. If you know JSX, SwiftUI-style builders, or XML UI files, the idea is similar: you write a tree-shaped description, and the runtime turns it into actual widgets.

On this page, a few words matter:

- **sigil** means special syntax that produces normal AIVI values,
- **widget tree** means the nested structure of your UI,
- **reconciliation** means patching an existing live tree instead of rebuilding every widget.

The sigil lowers into these core data shapes:

- `GtkNode = GtkElement Text (List GtkAttr) (List GtkNode) | GtkTextNode Text`
- `GtkAttr = GtkAttribute Text Text`
- helpers: `gtkElement`, `gtkTextNode`, `gtkAttr`
- `GtkSignalEvent` — a typed ADT of widget events

`GtkSignalEvent` variants include:

- `GtkClicked WidgetId Text`
- `GtkInputChanged WidgetId Text Text`
- `GtkActivated WidgetId Text`
- `GtkToggled WidgetId Text Bool`
- `GtkValueChanged WidgetId Text Float`
- `GtkKeyPressed WidgetId Text Text Text`
- `GtkFocusIn WidgetId Text`
- `GtkFocusOut WidgetId Text`
- `GtkUnknownSignal WidgetId Text Text Text Text`

The second field is the widget's `id="..."` name, or `""` when no `id` is set. That is why many apps match GTK events by widget name instead of comparing integer ids.

### Building widget trees

- `buildFromNode` accepts `<object>`, `<interface>`, or `<template>` roots and returns one `WidgetId`.
- `buildWithIds` accepts the same roots and returns `{ root: WidgetId, widgets: Map Text WidgetId }`, which is convenient when you want direct access to named widgets after building.
- For `<interface>` or `<template>`, the first nested `<object>` becomes the instantiated root.
- Object references via `ref` and `idref` are resolved against `id` attributes.
- `<child type="overlay">` and `<child type="controller">` are supported for overlay/controller wiring.
- Header-bar placement supports `<child type="title">` and `<child type="end">`; `start` is the default.
- Bare `<child>` without a `type` attribute is an error (`E1616`). Nest `<object>` elements directly inside the parent instead.

### Shorthand widget tags (preferred)

Tags starting with `Gtk`, `Adw`, or `Gsk` are shorthand for `<object class="...">`. They are the style most readers will want because they are shorter and closer to the widget names you see in GTK documentation.

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_02.aivi{aivi}


The parser lowers shorthand tags to the same IR as the verbose `<object>` form:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_03.aivi{aivi}


Attributes on shorthand tags become properties automatically, except for pass-through attributes such as `id` and `ref`.

| Attribute | Lowering |
| --- | --- |
| `label="Save"` | `prop:label` |
| `marginTop="12"` | `prop:margin-top` |
| `id="my-btn"` | `id` |
| `ref="btnRef"` | `ref` |
| `onClick={...}` | `signal:clicked` |
| `onInput={...}` | `signal:changed` |

### Which syntax to use

| Syntax style | Reach for it when... |
| --- | --- |
| shorthand tags such as `<GtkButton ... />` | day-to-day app code; this is the most readable default |
| verbose `<object class="...">` form | you need builder-style structure or want to mirror GTK builder docs closely |
| explicit `<signal ... />` tags | the signal name is clearer written explicitly or does not map neatly to sugar |

Inside `~<gtk>` sigils, the LSP can also help with tag-name completion, widget property completion, and snippets for construct-only properties.

### Signal sugar

Signal sugar works on both shorthand and verbose tags:

- `onClick={ Msg.Save }` → `signal:clicked`
- `onInput={ Msg.NameChanged }` → `signal:changed`
- `onActivate={ Msg.Submit }` → `signal:activate`
- `onToggle={ Msg.Toggled }` → `signal:toggled`
- `onValueChanged={ Msg.VolumeChanged }` → `signal:value-changed`
- `onFocusIn={ Msg.Focused }` → `signal:focus-enter`
- `onFocusOut={ Msg.Blurred }` → `signal:focus-leave`
- `<signal name="clicked" on={ Msg.Save } />` → the same binding path

Signal handler values must be compile-time expressions, such as constructors or constructor-like tags. Runtime lambdas are not valid here.

Inside `gtkApp`, the common case can use `toMsg: auto` instead of repeating a manual `GtkSignalEvent -> Option Msg` adapter. `auto` derives constructor-style signal routing from the current view tree and works best when each signal is either unique in the view or attached to a widget with an `id="..."` name.

### Runtime coverage and dynamic children

The runtime covers many common classes such as `GtkBox`, `GtkHeaderBar`, `AdwHeaderBar`, `AdwClamp`, `GtkLabel`, `GtkButton`, `GtkEntry`, `GtkImage`, `GtkDrawingArea`, `GtkScrolledWindow`, `GtkOverlay`, `GtkSeparator`, `GtkListBox`, and `GtkGestureClick`. Additional `Adw*` classes can be created dynamically when their GType is available.

Supported builder properties include layout and widget basics such as `margin-*`, `hexpand`, `vexpand`, `halign`, `valign`, `width-request`, `height-request`, `visible`, `tooltip-text`, `opacity`, style classes, and several class-specific fields.

Dynamic child lists can be expressed with `<each ...>` inside a GTK element:

- `<each items={items} as={item}> ... </each>`
- `items` must be a splice expression
- `as` must be an identifier splice
- the body must contain exactly one template node

`<each>` lowers to mapped child nodes and is flattened into the parent `children` list.

Uppercase or dotted GTK tags are treated as component calls instead of intrinsic widgets:

- `<ProjectRow ... />`
- `<Ui.ProjectRow ... />`

Component tags use **record-based lowering**: attributes become record fields and children become a `children` field. Signal sugar and `props` normalization do not apply there because the component function owns its own API.

GTK sigils also support **function-call tags** for local lowerCamel helpers that would be awkward to spell directly inside a sigil. A simple uppercase self-closing tag with positional arguments lowers to the same helper with a lowercased first letter:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_01.aivi{aivi}


Function-call tags:

- only apply to simple non-`Gtk*`/`Adw*`/`Gsk*` tags,
- use positional arguments instead of attributes,
- must be self-closing, and
- do not participate in component record lowering.

### Queue-based signal helpers

- `signalPoll : Unit -> Effect GtkError (Option GtkSignalEvent)` reads the next queued signal event, returning `None` when the queue is empty.
- `signalStream : Unit -> Effect GtkError (Recv GtkSignalEvent)` returns a receiver that emits events as they happen.
- `signalEmit` injects synthetic events, which is especially useful in tests or mock-driven flows.

For standard apps, `signalStream` is the usual choice. `signalPoll` is more useful for manual loops and debugging.

### Example: builder + shorthand

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_05.aivi{aivi}


### Example: builder + property sugar (verbose form)

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_06.aivi{aivi}


### Example: signal sugar with shorthand

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_07.aivi{aivi}


### Example: signal sugar with verbose form

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_08.aivi{aivi}


### Example: explicit `<signal>` tags

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_09.aivi{aivi}


### Example: dynamic list children with `<each>`

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_10.aivi{aivi}


### Example: consuming queued signal events (`signalPoll`)

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_11.aivi{aivi}


### Example: consuming signal events via `signalStream`

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_12.aivi{aivi}


## `gtkApp` — the standard app host

`gtkApp` is the recommended entry point for most GTK applications in AIVI. You give it a configuration record, and it handles init, startup, window creation, event ingestion, reconciliation, and the command/subscription flow described in [GTK App Architecture](./app_architecture.md).

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_13.aivi{aivi}


Internally, `gtkApp` performs: `init` → `appNew` → `windowNew` → `onStart` → `buildFromNode` → `windowSetChild` → `signalStream` → initial `subscriptions` → `windowPresent` → event loop with `toMsg`/`update` → reactive invalidation for changed source snapshots → `reconcileNode` → subscription refresh → command launch.

The helper surface includes:

- `AppStep { model, commands }`
- `auto` for common constructor-style `toMsg` routing
- command helpers: `commandNone`, `commandBatch`, `commandEmit`, `commandPerform`, `commandAfter`, `commandCancel`
- subscription helpers: `subscriptionNone`, `subscriptionBatch`, `subscriptionEvery`, `subscriptionSource`
- reactive helper: `computed`
- compatibility helpers: `noSubscriptions`, `appStep`, `appStepWith`, `liftAppUpdate`

`commandPerform` and `subscriptionSource` operate on `Msg` directly today (`run : Effect GtkError msg`, `open : Resource GtkError (Recv msg)`), which keeps many ordinary apps straightforward to write. `appStep` and `appStepWith` are optional shorthand for the same `{ model, commands }` record shape that `update` returns.

| Helper | Use it when... | Optional? |
| --- | --- | --- |
| `appStep`, `appStepWith` | you want shorter syntax for `{ model, commands }` | yes |
| `noSubscriptions` | the app has no timers or long-lived feeds | yes |
| `liftAppUpdate` | older code still returns only `model` from `update` | yes; mainly a migration helper |

`onStart` is the right place for one-time setup such as app CSS or action registration. Repeating timers, ongoing background work, and external feeds belong in commands or subscriptions.

For unusual window flags such as `decorated` or `hideOnClose`, keep `gtkApp` as the host and apply those settings from `onStart` with lower-level helpers such as `windowSetDecorated` and `windowSetHideOnClose`. If you need a fully custom lifecycle, use `signalStream`, `buildFromNode`, and `reconcileNode` directly rather than reaching for a second host API.

## `reconcileNode` — patching a live widget tree

`reconcileNode` diffs a new `GtkNode` tree against the live widget tree and applies the smallest changes it can:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_02.aivi{aivi}


It returns the root `WidgetId`: the same id when the root widget was patched in place, or a new id if the root widget type had to be rebuilt.

Properties are patched, CSS classes are diffed, signal handlers are reconnected when bindings change, and children are reconciled positionally.

### Full example

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_15.aivi{aivi}


## Diagnostics

- `E1612`: invalid `props` shape (must be a compile-time record literal)
- `E1613`: non-literal `props` field value
- `E1614`: invalid signal binding (`onClick`, `onInput`, `onActivate`, `onToggle`, `onValueChanged`, `onFocusIn`, `onFocusOut`, and `<signal ... on={...}>` require compile-time values)
- `E1615`: invalid `<each>` usage (requires `items={...}`, `as={...}`, and exactly one child template node)
- `E1616`: bare `<child>` without a `type` attribute (nest `<object>` elements directly inside the parent instead)
- `E1617`: invalid GTK function-call tag usage (function-call sugar must use positional arguments on a self-closing tag and cannot mix with attributes)
- Runtime signal-binding failures report the widget id/class, optional `id="..."` name, the bound handler, and the known supported signals for that class.
- `AdwPreferencesDialog` and `AdwPreferencesPage` validate page/group child compatibility before calling libadwaita so invalid hierarchies fail with an AIVI runtime error instead of only a GTK assertion.

## UI update pattern (state machine + events + repaint)

For drawing areas or custom stateful widgets, the pattern is still the same:

1. represent UI state as a model value,
2. convert GTK input into `Msg`,
3. update the model,
4. repaint or apply setters.

<<< ../../snippets/from_md/stdlib/ui/gtk4/ui_update_pattern.aivi{aivi}

For non-canvas widgets, use the same model/update approach but call setters such as `labelSetText`, `entrySetText`, or `widgetSetCss` instead of `drawAreaQueueDraw`.

## Compatibility with typed style data

`widgetSetCss` and `appSetCss` accept AIVI style records (`{ ... }`), so data from `aivi.ui` and `aivi.ui.layout` can be reused when styling GTK widgets or an entire app.
