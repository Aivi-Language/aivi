# `aivi.ui.gtk4`
## Native GTK4 Runtime Bindings

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the convenience module for GTK4-oriented native UI effects.
It exposes AIVI types/functions mapped directly to runtime native bindings.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

## Public API

<<< ../../snippets/from_md/stdlib/ui/gtk4/public_api.aivi{aivi}

## Native Mapping Table

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
| `windowNew` | `gtk4.windowNew` |
| `windowSetTitle` | `gtk4.windowSetTitle` |
| `windowSetChild` | `gtk4.windowSetChild` |
| `windowPresent` | `gtk4.windowPresent` |
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
| `trayIconNew` | `gtk4.trayIconNew` |
| `trayIconSetTooltip` | `gtk4.trayIconSetTooltip` |
| `trayIconSetVisible` | `gtk4.trayIconSetVisible` |
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

<<< ../../snippets/from_md/stdlib/ui/gtk4/example.aivi{aivi}

## GTK XML sigil (`~<gtk>...</gtk>`)

`aivi.ui.gtk4` also exposes typed constructors used by the GTK XML sigil:

- `GtkNode = GtkElement Text (List GtkAttr) (List GtkNode) | GtkTextNode Text`
- `GtkAttr = GtkAttribute Text Text`
- helpers: `gtkElement`, `gtkTextNode`, `gtkAttr`
- `GtkSignalEvent` — typed ADT with variants (second field is the widget's `id="..."` name, `""` if unset):
  - `GtkClicked WidgetId Text`
  - `GtkInputChanged WidgetId Text Text`
  - `GtkActivated WidgetId Text`
  - `GtkToggled WidgetId Text Bool`
  - `GtkValueChanged WidgetId Text Float`
  - `GtkKeyPressed WidgetId Text Text Text`
  - `GtkFocusIn WidgetId Text`
  - `GtkFocusOut WidgetId Text`
  - `GtkUnknownSignal WidgetId Text Text Text Text`

The parser lowers `~<gtk>...</gtk>` into those constructors.
Instantiate the resulting node tree with `buildFromNode` or `buildWithIds`.
`buildFromNode` accepts `<object>`, `<interface>`, or `<template>` roots and returns a single `WidgetId`.
`buildWithIds` accepts the same roots but returns `{ root: WidgetId, widgets: Map Text WidgetId }` — a record containing the root widget and a map from `id="..."` names to their `WidgetId` values. This eliminates the need for separate `widgetById` calls after building.
For `<interface>`/`<template>`, the first nested `<object>` becomes the instantiated root.
Object references via `ref`/`idref` are resolved against `id` attributes.
`<child type="overlay">` and `<child type="controller">` are supported for overlay/controller wiring.
Header-bar child placement is supported via `<child type="title">` and `<child type="end">` (`start` is the default).
Bare `<child>` without a `type` attribute is an error (E1616); nest `<object>` elements directly inside the parent instead.

### Shorthand widget tags (preferred)

Tags starting with `Gtk`, `Adw`, or `Gsk` are syntactic sugar for `<object class="...">`. Attributes on shorthand tags become properties automatically — no `props={{ }}` wrapper is needed. Signal sugar (`onClick`, `onInput`, etc.) works identically on shorthand tags.

```aivi
// Shorthand — preferred style
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <AdwActionRow title="Save AI Settings" />
    <GtkButton label="Save" onClick={ Msg.Save } />
  </GtkBox>
</gtk>
```

The parser lowers shorthand tags to the same IR as the verbose `<object>` form:

```aivi
// Equivalent verbose form
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="AdwActionRow" props={{ title: "Save AI Settings" }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={ Msg.Save } />
  </object>
</gtk>
```

**Attribute handling on shorthand tags:**

| Attribute        | Lowering                                           |
|:---------------- |:-------------------------------------------------- |
| `label="Save"`   | `prop:label` (camelCase normalized to kebab-case)  |
| `marginTop="12"` | `prop:margin-top`                                  |
| `id="my-btn"`    | `id` (pass-through, not a property)                |
| `ref="btnRef"`   | `ref` (pass-through, not a property)               |
| `onClick={...}`  | `signal:clicked` (signal sugar)                    |
| `onInput={...}`  | `signal:changed` (signal sugar)                    |

**LSP support:** Inside `~<gtk>` sigils, the LSP provides:
- **Tag name completion** — typing `<Gtk` or `<Adw` offers matching widget class names from the GIR index (350+ widgets from GTK4 and libadwaita).
- **Attribute completion** — after a tag name, offers property names for that widget class (including inherited properties).
- **Snippet insertion** — selecting a widget from completions inserts a snippet with tab stops for construct-only (mandatory) properties.

### Signal sugar

Signal sugar is supported on both shorthand and `<object>` tags, lowered to typed signal attrs:

- `onClick={ Msg.Save }` → `signal:clicked`
- `onInput={ Msg.Changed }` → `signal:changed`
- `onActivate={ Msg.Submit }` → `signal:activate`
- `onToggle={ Msg.Toggled }` → `signal:toggled`
- `onValueChanged={ Msg.VolumeChanged }` → `signal:value-changed`
- `onFocusIn={ Msg.Focused }` → `signal:focus-enter`
- `onFocusOut={ Msg.Blurred }` → `signal:focus-leave`
- `<signal name="clicked" on={ Msg.Save } />` → same binding path

Signal handler values must be compile-time expressions (for example constructor-like tags such as `Msg.Save`), not runtime lambdas.

### Runtime coverage

Current runtime coverage includes common classes such as `GtkBox`, `GtkHeaderBar`, `AdwHeaderBar`, `AdwClamp`, `GtkLabel`, `GtkButton`, `GtkEntry`, `GtkImage`, `GtkDrawingArea`, `GtkScrolledWindow`, `GtkOverlay`, `GtkSeparator`, `GtkListBox`, and `GtkGestureClick`. Additional `Adw*` classes are created dynamically when the runtime can resolve their GType.
Supported builder properties include layout/widget basics (`margin-*`, `hexpand`, `vexpand`, `halign`, `valign`, `width-request`, `height-request`, `visible`, `tooltip-text`, `opacity`, style classes), plus class-specific fields like `homogeneous`, `wrap`, `ellipsize`, `xalign`, `max-width-chars`, scrollbar policies, natural-propagation flags, `decoration-layout`, and `show-title-buttons`/`show-end-title-buttons`.

Dynamic child lists can be expressed with `<each ...>` inside a GTK element:

- `<each items={items} as={item}> ... </each>`
- `items` must be a splice expression.
- `as` must be an identifier splice.
- The `<each>` body must contain exactly one template node.

`<each>` lowers to mapped child nodes and is flattened into the parent `children` list.

Uppercase or dotted GTK tags are treated as component calls:

- `<Row ... />`
- `<Ui.Row ... />`

Component tags use **record-based lowering**: attributes become record fields and children become a `children` field. Signal sugar (`onClick`, `onInput`, …) and `props` normalization do **not** apply to component tags — the component function owns its API.

Lowering shape:

- `Row { id: "one", onClick: Save }` (single record argument)
- `Ui.Row { title: "Hello", children: [...] }` (children present)

This mirrors JSX/React conventions: each component is a function that receives a typed record of props.

Lowercase GTK tags continue to lower to `gtkElement`.

`props={ { ... } }` sugar on any tag expands to normalized GTK properties:

- `marginTop` becomes `prop:margin-top`
- `spacing` stays `prop:spacing`

In v0.1, `props` must be a compile-time record literal; dynamic `props={expr}` is a diagnostic.

`signalPoll : Unit -> Effect GtkError (Option GtkSignalEvent)` reads the next queued signal event (returns `None` when the queue is empty).
`signalStream : Unit -> Effect GtkError (Recv GtkSignalEvent)` returns a channel receiver that receives typed signal events as they fire — preferred over polling loops.
`signalEmit` is available for synthetic/manual event injection (useful in tests and mock-driven flows).

### Example: builder + shorthand (recommended)

```aivi
uiNode : GtkNode
uiNode =
  ~<gtk>
    <GtkBox orientation="vertical" spacing="12" marginTop="16">
      <GtkLabel label="Settings" />
    </GtkBox>
  </gtk>
```

### Example: builder + property sugar (verbose form)

```aivi
uiNode : GtkNode
uiNode =
  ~<gtk>
    <object class="GtkBox" props={ { orientation: "vertical", spacing: 12, marginTop: 16 } }>
      <object class="GtkLabel">
        <property name="label">Settings</property>
      </object>
    </object>
  </gtk>
```

### Example: signal sugar with shorthand (recommended)

```aivi
Msg = Save | NameChanged Text

formNode : GtkNode
formNode =
  ~<gtk>
    <GtkBox orientation="vertical" spacing="8">
      <GtkEntry onInput={ Msg.NameChanged } />
      <GtkButton label="Save" onClick={ Msg.Save } />
    </GtkBox>
  </gtk>
```

### Example: signal sugar with verbose form

```aivi
Msg = Save | NameChanged

formNode : GtkNode
formNode =
  ~<gtk>
    <object class="GtkBox" props={ { orientation: "vertical", spacing: 8 } }>
      <object class="GtkEntry" onInput={ Msg.NameChanged } />
      <object class="GtkButton" onClick={ Msg.Save }>
        <property name="label">Save</property>
      </object>
    </object>
  </gtk>
```

### Example: explicit `<signal>` tags

```aivi
Msg = Save

buttonNode : GtkNode
buttonNode =
  ~<gtk>
    <object class="GtkButton">
      <property name="label">Save</property>
      <signal name="clicked" on={ Msg.Save } />
    </object>
  </gtk>
```

### Example: dynamic list children with `<each>`

```aivi
items = ["A", "B", "C"]

listNode : GtkNode
listNode =
  ~<gtk>
    <object class="GtkBox" props={ { orientation: "vertical", spacing: 4 } }>
      <each items={items} as={item}>
          <object class="GtkLabel">
            <property name="label">{ item }</property>
          </object>
        </each>
    </object>
  </gtk>
```

### Example: consuming queued signal events (poll)

```aivi
nextMsg : Effect GtkError (Option Text)
nextMsg = do Effect {
  eventOpt <- signalPoll {}
  eventOpt match
    | None                              => pure None
    | Some (GtkClicked _ _)             => pure (Some "clicked")
    | Some (GtkInputChanged _ _ txt)    => pure (Some txt)
    | Some (GtkActivated _ _)           => pure (Some "activated")
    | Some (GtkToggled _ _ active)      => pure (Some (active | Bool.toString))
    | Some (GtkValueChanged _ _ val)    => pure (Some (val | Float.toString))
    | Some (GtkFocusIn _ _)             => pure (Some "focus-in")
    | Some (GtkFocusOut _ _)            => pure (Some "focus-out")
    | Some (GtkKeyPressed _ _ key _)    => pure (Some key)
    | Some (GtkUnknownSignal _ _ sig _ _) => pure (Some sig)
}
```

### Example: consuming signal events via `signalStream` (recommended)

```aivi
Msg = Save | NameChanged Text | Toggled Bool | VolumeChanged Float

toMsg : GtkSignalEvent -> Option Msg
toMsg = event =>
  event match
    | GtkClicked _ _            => Some Save
    | GtkInputChanged _ _ txt   => Some (NameChanged txt)
    | GtkToggled _ _ active     => Some (Toggled active)
    | GtkValueChanged _ _ val   => Some (VolumeChanged val)
    | _                         => None

runLoop : Effect GtkError Unit
runLoop = do Effect {
  events <- signalStream {}
  channel.forEach events (event =>
    toMsg event match
      | None     => pure {}
      | Some msg => handleMsg msg
  )
}
```

### `gtkApp` — Elm-architecture combinator

`gtkApp` encapsulates the entire GTK application lifecycle (init, window creation, event loop) into a single call. The user provides a configuration record and `gtkApp` handles the rest:

```aivi
gtkApp : {
  id:     Text,
  title:  Text,
  size:   (Int, Int),
  model:  s,
  view:   s -> GtkNode,
  toMsg:  GtkSignalEvent -> Option msg,
  update: msg -> s -> Effect GtkError s
} -> Effect GtkError Unit
```

Internally, `gtkApp` performs: `init` → `appNew` → `windowNew` → `buildFromNode` → `windowSetChild` → `signalStream` → `windowPresent` → event loop using `channel.recv` with `toMsg`/`update`. The GTK event loop is driven by `channel.recv`, which pumps GTK events internally via `g_main_context_iteration` — no separate `appRun` call is needed. On each state change, the `view` function is called with the new state and the resulting node tree is reconciled against the live widget tree via `reconcileNode`. If the root widget type changes, `gtkApp` automatically re-attaches the new root to the window.

### `reconcileNode` — vdom-style tree patching

`reconcileNode` diffs a new `GtkNode` tree against the live widget tree and applies minimal updates:

```aivi
reconcileNode : WidgetId -> GtkNode -> Effect GtkError WidgetId
```

Returns the root `WidgetId` — same as input when the root was patched in-place, or a new id if the root widget type changed and was rebuilt. Callers should use the returned id for subsequent reconciliation and re-attach to the window if it changed.

Properties are patched, CSS classes are diffed (add/remove), and signal handlers are disconnected and reconnected when bindings change. Children are reconciled positionally: same-class children are patched, different-class children are replaced, excess children are removed, and new children are appended.

#### Full example

```aivi
Msg = TitleChanged Text | BodyChanged Text | Save

editorView : { title: Text, body: Text } -> GtkNode
editorView = state => ~<gtk>
  <GtkBox orientation="vertical" spacing="8">
    <GtkEntry id="titleInput" placeholderText="Title" />
    <GtkEntry id="bodyInput" placeholderText="Body" />
    <GtkButton label="Save" onClick={ Msg.Save } />
  </GtkBox>
</gtk>

toMsg : GtkSignalEvent -> Option Msg
toMsg = event =>
  event match
    | GtkInputChanged _ "titleInput" txt => Some (TitleChanged txt)
    | GtkInputChanged _ "bodyInput" txt  => Some (BodyChanged txt)
    | GtkClicked _ _                     => Some Save
    | _                                  => None

update : Msg -> { title: Text, body: Text } -> Effect GtkError { title: Text, body: Text }
update = msg => state =>
  msg match
    | TitleChanged txt => pure (state <| { title: txt })
    | BodyChanged txt  => pure (state <| { body: txt })
    | Save             => do Effect { _ <- saveNote state; pure state }

main : Effect GtkError Unit
main = gtkApp {
  id:     "com.example.notepad",
  title:  "Notepad",
  size:   (640, 480),
  model:  { title: "", body: "" },
  view:   editorView,
  toMsg:  toMsg,
  update: update
}
```

### Diagnostics

- `E1612`: invalid `props` shape (must be compile-time record literal).
- `E1613`: non-literal `props` field value.
- `E1614`: invalid signal binding (`onClick`/`onInput`/`onActivate`/`onToggle`/`onValueChanged`/`onFocusIn`/`onFocusOut`/`<signal ... on={...}>` requires compile-time values).
- `E1615`: invalid `<each>` usage (requires `items={...}`, `as={...}`, and exactly one child template node).

## UI update pattern (state machine + events + repaint)

You can drive GTK updates from an AIVI model/update loop:

1. represent UI state as a model value,
2. model valid transitions with `machine`,
3. convert GTK input into `Msg`,
4. call `drawAreaQueueDraw` (or widget setters) when state changes.

<<< ../../snippets/from_md/stdlib/ui/gtk4/ui_update_pattern.aivi{aivi}

For non-canvas widgets, do the same model/update step but call setters directly (`labelSetText`, `entrySetText`, `widgetSetCss`, etc.) instead of `drawAreaQueueDraw`.

## Compatibility

`widgetSetCss` and `appSetCss` accept AIVI style records (`{ }`) so your existing `aivi.ui`/`aivi.ui.layout` CSS-style values can be reused with GTK widgets/app styling.

## Lucide SVG workflow (GNOME GTK4 target)

For production packaging, prefer `imageNewFromResource`/`imageSetResource` with compiled GResources (for example `/com/example/YourApp/icons/lucide/home.svg`), and register your `.gresource` bundle before loading images.
`imageNewFromFile`/`imageSetFile` remain available for local prototyping from disk paths.
