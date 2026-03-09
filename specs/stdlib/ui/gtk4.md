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
- `GtkGrid` children use `<child type="col,row">` or `<child type="col,row,colspan,rowspan">` to specify the grid position. Defaults are col=0, row=0, colspan=1, rowspan=1.
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

The runtime covers the following GTK4 classes directly:

**Layout containers:** `GtkBox`, `GtkHeaderBar`, `AdwHeaderBar`, `AdwClamp`, `GtkGrid`, `GtkAspectFrame`, `GtkPaned`, `GtkFrame`, `GtkExpander`, `GtkFlowBox`, `GtkNotebook`, `GtkOverlay`, `GtkScrolledWindow`, `GtkStack`, `GtkRevealer`, `GtkActionBar`, `GtkCenterBox`, `GtkSearchBar`

**Interactive widgets:** `GtkButton`, `GtkCheckButton`, `GtkToggleButton`, `GtkSwitch`, `GtkEntry`, `GtkPasswordEntry`, `GtkSearchEntry`, `GtkCalendar`, `GtkTextView`, `GtkScale`, `GtkRange`, `GtkSpinButton`, `GtkDropDown`, `GtkComboBoxText`, `GtkColorDialogButton`, `GtkFontDialogButton`, `GtkMenuButton`, `GtkLinkButton`, `GtkSpinner`, `GtkProgressBar`, `GtkStackSwitcher`

**Display widgets:** `GtkLabel`, `GtkImage`, `GtkPicture`, `GtkDrawingArea`, `GtkSeparator`, `GtkListBox`

**Event controllers:** `GtkGestureClick`

Additional `Adw*` classes can be created dynamically when their GType is available (see the Adw coverage table in this file).

Widget signals supported per class:

| Class | Signals |
| --- | --- |
| `GtkButton`, `GtkLinkButton` | `clicked` |
| `GtkEntry`, `GtkPasswordEntry` | `changed`, `activate` |
| `GtkSearchEntry` | `changed`, `search-changed` |
| `GtkCheckButton` | `toggled` |
| `GtkToggleButton` | `toggled` |
| `GtkSwitch` | `notify::active` |
| `GtkScale`, `GtkRange` | `value-changed` |
| `GtkSpinButton` | `value-changed` |
| `GtkComboBoxText` | `changed` |
| `GtkColorDialogButton` | `notify::rgba` |
| `GtkFontDialogButton` | `notify::font-desc` |
| `GtkDropDown` | `notify::selected` |
| `GtkCalendar` | `day-selected` |
| `GtkNotebook` | `switch-page` |
| `AdwEntryRow`, `AdwPasswordEntryRow` | `changed` |
| `AdwSwitchRow` | `toggled` |
| `AdwOverlaySplitView` | `notify::show-sidebar` |

Supported builder properties include layout and widget basics such as `margin-*`, `hexpand`, `vexpand`, `halign`, `valign`, `width-request`, `height-request`, `visible`, `tooltip-text`, `opacity`, style classes, and several class-specific fields.

`GtkGrid` child placement is specified via `<child type="col,row">` or `<child type="col,row,colspan,rowspan">`. `GtkSpinButton` is constructed with `min`, `max`, and `step` attributes. `GtkComboBoxText` accepts a `strings` attribute (newline-separated items) and is supported for compatibility, but prefer `GtkDropDown` for new code since `GtkComboBoxText` is deprecated in GTK4.

`GtkColorDialogButton` accepts an optional `rgba` attribute in `"r,g,b"` or `"r,g,b,a"` format (float components in 0–1) to set the initial colour. When the user selects a colour, a `notify::rgba` signal fires and the event payload is the new RGBA value as a `"r,g,b,a"` comma-separated string.

`GtkFontDialogButton` accepts an optional `font-desc` attribute with a Pango font description string (e.g. `"Sans Bold 12"`) to set the initial font. When the user selects a font, a `notify::font-desc` signal fires and the event payload is the Pango description string.

`GtkSearchBar` accepts `search-mode` and `show-close-button` boolean attributes. Its first `GtkSearchEntry`, `GtkEntry`, or `GtkPasswordEntry` child is automatically connected so keyboard search capture works without extra wiring.

`GtkActionBar` and `GtkCenterBox` use `<child type="center">` and `<child type="end">` to place center and trailing children; children without a `type` attribute are treated as the start slot.

`GtkPicture` accepts `file`/`filename`, `resource`, `content-fit` (`fill`, `contain`, `cover`, or `scale-down`), `can-shrink`, and `alternative-text`/`alt`.

`GtkCalendar` accepts `show-heading`, `show-day-names`, and `show-week-numbers` boolean attributes.

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
- derived helpers: `derive`, `memo`, `readDerived`
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

## libadwaita types not directly creatable in widget trees

Some libadwaita types exist in the GObject type registry but are not appropriate to use as standalone `<object>` elements in a `~<gtk>` widget tree. Attempting to use them directly yields a runtime error with a descriptive message explaining the alternative.

| Type | Category | Correct approach |
| --- | --- | --- |
| `AdwAnimation` | Abstract base class | Use `AdwTimedAnimation` or `AdwSpringAnimation` |
| `AdwAnimationTarget` | Abstract base class | Use `AdwCallbackAnimationTarget` or `AdwPropertyAnimationTarget` |
| `AdwTimedAnimation` | Programmatic animation object | Created and started programmatically; set the `target` and `value-from`/`value-to` properties |
| `AdwSpringAnimation` | Programmatic animation object | Created and started programmatically; spring physics variant of `AdwTimedAnimation` |
| `AdwCallbackAnimationTarget` | Programmatic animation target | Calls a native callback on each animation frame; created programmatically |
| `AdwPropertyAnimationTarget` | Programmatic animation target | Animates a GObject property by name; created programmatically |
| `AdwEnumListItem` | Model-internal item | Items are owned by `AdwEnumListModel`; use `AdwEnumListModel` directly |
| `AdwEnumListModel` | List model | Exposes all values of a GLib enum as a list; pass as the `model` property of a selection model or `GtkDropDown` |
| `AdwLeafletPage` | Parent-managed auxiliary | Add child widgets directly inside `AdwLeaflet` |
| `AdwSqueezerPage` | Parent-managed auxiliary | Add child widgets directly inside `AdwSqueezer` |
| `AdwTabPage` | Parent-managed auxiliary | Add child widgets directly inside `AdwTabView` |
| `AdwViewStackPage` | Parent-managed auxiliary | Add child widgets directly inside `AdwViewStack` |
| `AdwViewStackPages` | Internal list model | Owned by `AdwViewStack`; not part of the widget tree |
| `AdwToast` | Notification object | Created programmatically and added to `AdwToastOverlay` via `adw_toast_overlay_add_toast()` |
| `AdwBreakpoint` | Responsive layout helper | Created programmatically; attached to an `AdwBreakpointBin` or window to apply property changes at given width thresholds |
| `AdwBreakpointCondition` | Condition descriptor | Created via `adw_breakpoint_condition_parse()`; set as the `condition` property of `AdwBreakpoint` |

`AdwStyleManager` is the singleton for application-wide styling. It **can** appear as a named `<object>` in a widget tree if you need to reference it by id (e.g., to read the `dark` property), but it is fetched via `adw_style_manager_get_default()` rather than constructed—so it is safe to reference multiple times without creating a second instance.

## GTK core types not directly creatable in widget trees

Several GObject types in GTK4 exist in the type registry but are not appropriate as standalone `<object>` elements in a `~<gtk>` widget tree. They fall into distinct groups; understanding which group a type belongs to usually makes the right approach clear.

### Abstract base classes and interfaces

These types cannot be instantiated. They exist only as compile-time type constraints or as the root of an inheritance hierarchy.

| Type | Notes |
| --- | --- |
| `GtkWidget` | Root abstract widget class; use any concrete subclass |
| `GtkRange` | Concrete base for sliders; use `GtkScale` by name for clarity (both are accepted) |
| `GtkEventController` | Abstract base for all event controllers; use a concrete subclass |
| `GtkGesture` | Abstract base for gesture recognizers; use `GtkGestureClick`, `GtkGestureDrag`, etc. |
| `GtkFilter` | Abstract base for `GtkCustomFilter`, `GtkStringFilter`, etc. |
| `GtkSorter` | Abstract base for `GtkCustomSorter`, `GtkStringSorter`, etc. |
| `GtkExpression` | Abstract base for `GtkPropertyExpression`, `GtkConstantExpression`, etc. |
| `GtkLayoutManager` | Abstract base for layout managers; use concrete subclasses or the default |
| `GtkSelectionModel` | Interface; use `GtkNoSelection`, `GtkSingleSelection`, or `GtkMultiSelection` |
| `GtkOrientable` | Interface implemented by orientable widgets; not instantiable on its own |
| `GtkScrollable` | Interface implemented by scrollable widgets; not instantiable on its own |
| `GtkEditable` | Interface implemented by text-input widgets; not instantiable on its own |
| `GtkBuildable` | Interface automatically implemented by all GTK objects; not instantiable on its own |

### List model and selection infrastructure

These types act as data models or selection wrappers and are set as **properties** on list-displaying widgets such as `GtkDropDown`, `GtkListView`, and `GtkColumnView`. They are not widget-tree nodes.

| Type | Correct approach |
| --- | --- |
| `GtkStringList` | Pass as the `model` property of `GtkDropDown` or a selection model |
| `GtkFilterListModel` | Wrap another model with a `GtkFilter` and pass as a model property |
| `GtkSortListModel` | Wrap another model with a `GtkSorter` and pass as a model property |
| `GtkSliceListModel` | Wraps a model to expose a fixed window; pass as a model property |
| `GtkFlattenListModel` | Flattens a nested model; pass as a model property |
| `GtkNoSelection` | Selection wrapper; pass as the `model` property of a list view |
| `GtkSingleSelection` | Selection wrapper; pass as the `model` property of a list view |
| `GtkMultiSelection` | Selection wrapper; pass as the `model` property of a list view |
| `GtkListItem` | Created and recycled internally by `GtkListItemFactory`; never construct directly |
| `GtkListItemFactory` | Abstract base; use `GtkSignalListItemFactory` or `GtkBuilderListItemFactory` |

### Event controllers and gestures

Event controllers and gestures are attached to a widget with `<child type="controller">`, not placed as siblings or children in the normal tree. `GtkGestureClick` is the one gesture that is additionally tracked internally for click signals; all others are wired up purely through `<child type="controller">` and native GTK signals.

| Type | Notes |
| --- | --- |
| `GtkEventControllerKey` | Add with `<child type="controller">` on the target widget |
| `GtkEventControllerFocus` | Add with `<child type="controller">` on the target widget |
| `GtkEventControllerMotion` | Add with `<child type="controller">` on the target widget |
| `GtkEventControllerScroll` | Add with `<child type="controller">` on the target widget |
| `GtkEventControllerLegacy` | Add with `<child type="controller">` on the target widget |
| `GtkGestureDrag` | Add with `<child type="controller">` on the target widget |
| `GtkGestureLongPress` | Add with `<child type="controller">` on the target widget |
| `GtkGestureSwipe` | Add with `<child type="controller">` on the target widget |
| `GtkGestureZoom` | Add with `<child type="controller">` on the target widget |
| `GtkGestureRotate` | Add with `<child type="controller">` on the target widget |
| `GtkGesturePan` | Add with `<child type="controller">` on the target widget |
| `GtkGestureStylus` | Add with `<child type="controller">` on the target widget |
| `GtkShortcutController` | Add with `<child type="controller">` on the target widget |
| `GtkDropControllerMotion` | Add with `<child type="controller">` on the target widget |

### Adjustment

`GtkAdjustment` is a value-range object. It is set as a **property** (`adjustment`) on widgets such as `GtkSpinButton`, `GtkScale`, `GtkScrollbar`, and `GtkScrolledWindow`. It is not a widget itself.

### Text buffer model items

These types are created and owned by `GtkTextBuffer`. They are not widget-tree nodes.

| Type | Notes |
| --- | --- |
| `GtkTextTag` | Create via `GtkTextTagTable` or the tag table of a `GtkTextBuffer` |
| `GtkTextTagTable` | Set as the `buffer` property of `GtkTextView`; not a widget |
| `GtkTextMark` | Created by `GtkTextBuffer`; not a widget |
| `GtkTextChildAnchor` | Created by `GtkTextBuffer`; used to embed widgets in text |

### Deprecated cell-renderer and tree-model infrastructure

GTK4 deprecated the `GtkTreeView` column-cell architecture. The following types belong to it and should not appear in new widget trees. Use `GtkColumnView` with `GtkListItemFactory` instead.

| Type | Status |
| --- | --- |
| `GtkCellRenderer` | Deprecated; abstract base, use `GtkColumnView` |
| `GtkCellRendererText` | Deprecated; use `GtkColumnView` + `GtkLabel` in factory |
| `GtkCellRendererToggle` | Deprecated; use `GtkColumnView` + `GtkCheckButton` in factory |
| `GtkCellRendererPixbuf` | Deprecated; use `GtkColumnView` + `GtkImage` in factory |
| `GtkCellRendererProgress` | Deprecated; use `GtkColumnView` + `GtkProgressBar` in factory |
| `GtkCellRendererSpin` | Deprecated |
| `GtkCellRendererCombo` | Deprecated |
| `GtkCellRendererAccel` | Deprecated |
| `GtkTreeModel` | Deprecated interface; use `GtkNoSelection`/`GtkSingleSelection` + list model |
| `GtkListStore` | Deprecated; use `GtkStringList` or a custom `GListModel` |
| `GtkTreeStore` | Deprecated; use `GtkTreeListModel` |

### Shortcut and action infrastructure

Shortcuts and actions are wired up programmatically or via `<child type="controller">` on a `GtkShortcutController`. They are not standalone widget-tree elements.

| Type | Notes |
| --- | --- |
| `GtkShortcut` | Add to a `GtkShortcutController` |
| `GtkShortcutAction` | Abstract base; use `GtkActivateAction`, `GtkSignalAction`, `GtkNamedAction`, etc. |
| `GtkShortcutTrigger` | Abstract base; use `GtkKeyvalTrigger`, `GtkMnemonicTrigger`, etc. |
| `GtkActivateAction` | Singleton; use `gtk_activate_action_get()` |
| `GtkSignalAction` | Created programmatically; not a widget |
| `GtkNamedAction` | Created programmatically; not a widget |
| `GtkNothingAction` | Singleton |
| `GtkMnemonicAction` | Singleton |
| `GtkKeyvalTrigger` | Created programmatically; not a widget |
| `GtkMnemonicTrigger` | Created programmatically; not a widget |
| `GtkAlternativeTrigger` | Created programmatically; not a widget |
| `GtkNeverTrigger` | Singleton |
| `GtkAnyTrigger` | Singleton |

### Layout manager infrastructure

Layout managers are set as the `layout-manager` property of a container widget. They are not widget-tree nodes and do not appear as `<object>` elements.

| Type | Notes |
| --- | --- |
| `GtkBoxLayout` | Default for `GtkBox`; override via `layout-manager` property |
| `GtkGridLayout` | Default for `GtkGrid`; override via `layout-manager` property |
| `GtkFixedLayout` | Set via `layout-manager` property for absolute positioning |
| `GtkBinLayout` | Set via `layout-manager` property for single-child containers |
| `GtkOverlayLayout` | Default for `GtkOverlay`; not separately creatable |
| `GtkConstraintLayout` | Set via `layout-manager` property for constraint-based sizing |
| `GtkConstraint` | Added to a `GtkConstraintLayout`; not a widget |
| `GtkConstraintGuide` | Added to a `GtkConstraintLayout`; not a widget |

### Filter, sorter, and expression implementations

These types implement GTK4's data-model pipeline. They are passed as **properties** on list models (`GtkFilterListModel`, `GtkSortListModel`) and are not widget-tree nodes. In AIVI, you build filter/sort logic with ordinary AIVI functions passed to the constructor helpers rather than constructing GTK expression objects directly.

| Type | Usage |
| --- | --- |
| `GtkCustomFilter` | Pass a predicate function to `GtkFilterListModel.filter` |
| `GtkStringFilter` | Filter by string match; pass to `GtkFilterListModel.filter` |
| `GtkBoolFilter` | Filter by a `GtkExpression` that yields a boolean; pass to `GtkFilterListModel.filter` |
| `GtkMultiFilter` | Abstract base for combining filters |
| `GtkEveryFilter` | AND-combination of multiple filters |
| `GtkAnyFilter` | OR-combination of multiple filters |
| `GtkCustomSorter` | Pass a comparison function to `GtkSortListModel.sorter` |
| `GtkStringSorter` | Sort by a string-valued `GtkExpression`; pass to `GtkSortListModel.sorter` |
| `GtkNumericSorter` | Sort by a numeric `GtkExpression`; pass to `GtkSortListModel.sorter` |
| `GtkMultiSorter` | Composes multiple sorters in priority order |
| `GtkPropertyExpression` | Reads a GObject property by name; internal to GTK4 binding system |
| `GtkConstantExpression` | Wraps a constant value; internal to GTK4 binding system |
| `GtkCClosureExpression` | Wraps a native closure; internal to GTK4 binding system |
| `GtkObjectExpression` | Yields a GObject instance; internal to GTK4 binding system |

`GtkExpression` and its subtypes are part of GTK4's internal binding/property-notification system. In AIVI you express the same logic with pure functions and reactive derivations — you do not construct `GtkExpression` objects directly.

### List item factory implementations

`GtkListItemFactory` (abstract) is listed above. The concrete implementations:

| Type | Notes |
| --- | --- |
| `GtkSignalListItemFactory` | Emits `setup` and `bind` signals that a factory handler must connect; wired up programmatically |
| `GtkBuilderListItemFactory` | Uses a GTK Builder XML template; prefer the `~<gtk>` sigil over raw Builder XML |

For simple read-only lists, prefer `GtkListBox` with `<each>` dynamic children. `GtkListView`, `GtkColumnView`, and `GtkGridView` remain future work in AIVI's widget-tree runtime today, so the factory types are documented here as supporting infrastructure rather than currently-usable widget-tree surface.

### Concrete list widgets not yet exposed in widget trees

These are real GTK widgets, but AIVI v0.1 does not yet expose them directly in `~<gtk>` widget trees. Attempting to use them yields a runtime error that points back to `GtkListBox` + `<each>` for simple list UIs today.

| Type | Notes |
| --- | --- |
| `GtkListView` | Virtualized single-column list view; future work for a dedicated model/factory surface |
| `GtkColumnView` | Multi-column virtualized list view; future work for a dedicated model/factory surface |
| `GtkGridView` | Virtualized grid/list view; future work for a dedicated model/factory surface |

### Column view auxiliary types

`GtkColumnView` is a concrete GTK widget, but AIVI does not yet expose it in widget trees today. Its auxiliary types are not widget-tree nodes.

| Type | Notes |
| --- | --- |
| `GtkColumnViewColumn` | Added to `GtkColumnView` programmatically; not a widget-tree element |
| `GtkColumnViewRow` | Internal row object; managed by `GtkColumnView` |
| `GtkColumnViewCell` | Internal cell object; managed by the column factory |
| `GtkColumnViewSorter` | Wraps column sort state; obtained via `GtkColumnView.sorter` property |

### Media streaming helpers

`GtkMediaControls` and `GtkVideo` are concrete GTK widgets, but AIVI does not yet expose them in widget trees today. Their backing stream types are not widget-tree nodes.

| Type | Notes |
| --- | --- |
| `GtkMediaStream` | Abstract base for audio/video streams; set as the `stream` property of `GtkVideo` or `GtkMediaControls` |
| `GtkMediaFile` | Concrete `GtkMediaStream` for file/URI sources; created programmatically |

### Top-level windows and dialogs not exposed in widget trees

These GTK surfaces are managed either by AIVI's `windowNew` / `gtkApp` API or by imperative dialog presentation code. They are not used as direct `<object class="...">` entries inside a `~<gtk>` widget tree today.

| Type | Notes |
| --- | --- |
| `GtkWindow` | Top-level window; create via `windowNew` or `gtkApp`, not as a child widget node |
| `GtkApplicationWindow` | Application-managed top-level window; AIVI currently uses `windowNew` / `gtkApp` rather than direct `GtkApplicationWindow` construction |
| `GtkAboutDialog` | Top-level about dialog; prefer `AdwAboutDialog` / `AdwAboutWindow` in AIVI apps |
| `GtkDialog` | Legacy dialog base; prefer `GtkAlertDialog` or libadwaita dialogs |
| `GtkMessageDialog` | Legacy message dialog; prefer `GtkAlertDialog` or `AdwAlertDialog` |
| `GtkAlertDialog` | Asynchronous dialog API object; created and presented programmatically, not embedded in widget trees |
| `GtkShortcutsWindow` | Top-level shortcuts help window; prefer `AdwShortcutsDialog` |

### Popovers and menu popups

These are popup/menu surfaces that GTK presents transiently. AIVI does not yet expose them directly in widget trees today.

| Type | Notes |
| --- | --- |
| `GtkPopover` | Bubble popup surface; today prefer `GtkMenuButton` with a menu model or a custom button/dialog flow |
| `GtkPopoverMenu` | Menu-oriented `GtkPopover`; prefer `GtkMenuButton` + `GMenuModel` |
| `GtkPopoverMenuBar` | Menu bar driven by a `GMenuModel`; not yet exposed as a direct widget-tree surface |

### File chooser helpers and dialogs

GTK4's newer file APIs are asynchronous objects, while the older chooser widgets/dialogs are legacy surfaces. AIVI v0.1 does not yet expose file chooser APIs in the stdlib UI surface.

| Type | Notes |
| --- | --- |
| `GtkFileDialog` | Asynchronous file chooser object; create and present programmatically |
| `GtkFileChooserDialog` | Legacy file chooser dialog; prefer `GtkFileDialog` when working directly with GTK |
| `GtkFileChooserNative` | Legacy native file chooser wrapper; prefer `GtkFileDialog` |
| `GtkFileChooserWidget` | Legacy embeddable file chooser widget; not exposed in AIVI widget trees |

### Builder-only and singleton types

| Type | Notes |
| --- | --- |
| `GtkBuilder` | XML-based UI builder; use `buildFromNode` or the `~<gtk>` sigil instead |
| `GtkSettings` | Global singleton for GTK settings; fetched via `gtk_settings_get_default()`, not constructed |
| `GtkStyleContext` | Attached to widgets internally; do not create directly; use `widgetSetCss` |
| `GtkCssProvider` | Applied programmatically via `appSetCss` or `widgetSetCss`; not a widget |

## GIO action and menu infrastructure

GIO actions and menus are wired up programmatically, typically from the `onStart` hook in `gtkApp`. They are not widget-tree nodes and do not appear as `<object>` elements in a `~<gtk>` sigil.

| Type | Notes |
| --- | --- |
| `GApplication` / `GtkApplication` | Managed by `appNew` / `gtkApp`; do not create separately |
| `GSimpleAction` | Stateless or stateful action; add to a `GActionMap` via `g_action_map_add_action()` |
| `GActionGroup` | Interface grouping actions; implemented by `GApplication` and `GtkWidget` |
| `GActionMap` | Interface for adding/removing actions; implemented by `GApplication` |
| `GMenuModel` | Abstract read-only menu description; subclassed by `GMenu` |
| `GMenu` | Mutable menu model; build from `onStart` and attach to a `GtkMenuButton` via `menuButtonSetMenuModel` |
| `GMenuItem` | One entry in a `GMenu`; add with `g_menu_append_item()` |
| `GActionEntry` | C-struct helper for bulk action registration; not a GObject |

The practical pattern for keyboard shortcuts and application menus is:

1. Create a `GSimpleAction` in `onStart`.
2. Connect its `activate` signal to an AIVI command.
3. Add it to the `GApplication` action map so it can be addressed by name (e.g. `"app.quit"`).
4. Optionally set a `GtkShortcutController` on the window with the relevant trigger.

## Printing infrastructure

GTK4 printing is driven by `GtkPrintOperation` which opens the native print dialog. These types are not widget-tree nodes and are not part of the `~<gtk>` sigil surface.

> **AIVI v0.1 status:** Printing is not in the v0.1 stdlib scope. The types are documented here so you can recognise them if they appear in GTK documentation and understand why they are absent from AIVI's API surface.

| Type | Notes |
| --- | --- |
| `GtkPrintOperation` | Top-level object that runs the print dialog; created programmatically |
| `GtkPrintDialog` | Asynchronous print dialog object; created programmatically |
| `GtkPrinter` | Represents a physical or virtual printer; not constructed directly |
| `GtkPrintJob` | Represents one queued print job; created/programmed via the print API |
| `GtkPrintContext` | Passed to the `draw-page` signal handler during a print run |
| `GtkPageSetup` | Describes paper size, margins, and orientation |
| `GtkPageSetupUnixDialog` | Legacy Unix-only page setup dialog; not part of the widget-tree surface |
| `GtkPaperSize` | Identifies a paper format (ISO/custom); set on `GtkPageSetup` |
| `GtkPrintSettings` | Key/value store for printer-specific settings |
| `GtkPrintUnixDialog` | Legacy Unix-only print dialog; not part of the widget-tree surface |

## GTK widget types removed in GTK 4.10

The following GTK widget types were deprecated in earlier releases and removed in GTK 4.10. The runtime returns a descriptive error if you try to use them so that migration is clear.

| Removed type | Replacement |
| --- | --- |
| `GtkColorButton` | Use `GtkColorDialogButton` (GTK 4.10+) |
| `GtkFontButton` | Use `GtkFontDialogButton` (GTK 4.10+) |
| `GtkAppChooserButton` | Removed with no direct GTK4 replacement; use OS file manager integration or application-specific logic |
| `GtkAppChooserDialog` | Same as above |
| `GtkAppChooserWidget` | Same as above |

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
