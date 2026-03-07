# `aivi.ui.gtk4`
## GTK & libadwaita Runtime for Native Apps

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the runtime module for native desktop apps built with GTK4 and libadwaita. It provides the types and functions behind widget trees, GTK signal events, reconciliation, and the `gtkApp` host.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

Start with [Native GTK & libadwaita Apps](./native_gtk_apps.md) for the high-level guide and [GTK App Architecture](./app_architecture.md) for the event-loop details. This page is the practical API reference.

## How UI effects fit into capabilities

GTK runtime operations use the `ui` capability family:

- widget and window construction/presentation → `ui.window`,
- `signalPoll`, `signalStream`, `signalEmit` → `ui.signal`,
- clipboard helpers → `ui.clipboard`,
- desktop notification helpers → `ui.notification`,
- `gtkApp` → the coarse-grained `ui` entry point.

These capabilities describe where native UI effects live in AIVI's effect system. They do not create a separate “special UI language”.

## Essential API for first apps

Most single-window GTK apps only need this smaller subset:

- `gtkApp` — the standard event-loop host
- `GtkNode` and `GtkSignalEvent` — widget-tree and input-event types
- `~<gtk>...</gtk>` — the usual way to build the view tree
- timer helpers such as `commandAfter` or `subscriptionEvery`
- `reconcileNode` only indirectly, because `gtkApp` calls it for you

The rest of this page is still important reference material, but you do not need to memorize all of it before building a first app.

## Public API

<<< ../../snippets/from_md/stdlib/ui/gtk4/public_api.aivi{aivi}

## Native mapping table

The runtime forwards these AIVI functions to their native GTK or libadwaita counterparts:

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

```aivi
// Shorthand — the most readable style for day-to-day app code.
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <AdwActionRow title="Save AI Settings" />
    <GtkButton label="Save" onClick={ Msg.Save } />
  </GtkBox>
</gtk>
```

The parser lowers shorthand tags to the same IR as the verbose `<object>` form:

```aivi
// Equivalent verbose form.
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="AdwActionRow" props={{ title: "Save AI Settings" }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={ Msg.Save } />
  </object>
</gtk>
```

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
- `onInput={ Msg.Changed }` → `signal:changed`
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

- `<Row ... />`
- `<Ui.Row ... />`

Component tags use **record-based lowering**: attributes become record fields and children become a `children` field. Signal sugar and `props` normalization do not apply there because the component function owns its own API.

### Queue-based signal helpers

- `signalPoll : Unit -> Effect GtkError (Option GtkSignalEvent)` reads the next queued signal event, returning `None` when the queue is empty.
- `signalStream : Unit -> Effect GtkError (Recv GtkSignalEvent)` returns a receiver that emits events as they happen.
- `signalEmit` injects synthetic events, which is especially useful in tests or mock-driven flows.

For standard apps, `signalStream` is the usual choice. `signalPoll` is more useful for manual loops and debugging.

### Example: builder + shorthand

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

### Example: signal sugar with shorthand

```aivi
Msg = Save | NameChanged Text

formNode : GtkNode
formNode =
  ~<gtk>
    <GtkBox orientation="vertical" spacing="8">
      // The entry sends its new text as a message.
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

### Example: consuming queued signal events (`signalPoll`)

```aivi
nextMsg : Effect GtkError (Option Text)
nextMsg = do Effect {
  eventOpt <- signalPoll {}
  eventOpt match
    | None                                => pure None
    | Some (GtkClicked _ _)               => pure (Some "clicked")
    | Some (GtkInputChanged _ _ txt)      => pure (Some txt)
    | Some (GtkActivated _ _)             => pure (Some "activated")
    | Some (GtkToggled _ _ active)        => pure (Some (active | Bool.toString))
    | Some (GtkValueChanged _ _ val)      => pure (Some (val | Float.toString))
    | Some (GtkFocusIn _ _)               => pure (Some "focus-in")
    | Some (GtkFocusOut _ _)              => pure (Some "focus-out")
    | Some (GtkKeyPressed _ _ key _)      => pure (Some key)
    | Some (GtkUnknownSignal _ _ sig _ _) => pure (Some sig)
}
```

### Example: consuming signal events via `signalStream`

```aivi
Msg = Save | NameChanged Text | Toggled Bool | VolumeChanged Float

toMsg : GtkSignalEvent -> Option Msg
toMsg = event =>
  event match
    | GtkClicked _ _          => Some Save
    | GtkInputChanged _ _ txt => Some (NameChanged txt)
    | GtkToggled _ _ active   => Some (Toggled active)
    | GtkValueChanged _ _ val => Some (VolumeChanged val)
    | _                       => None

runLoop : Effect GtkError Unit
runLoop = do Effect {
  events <- signalStream {}
  channel.forEach events (event =>
    // Convert low-level widget events into domain messages.
    toMsg event match
      | None     => pure {}
      | Some msg => handleMsg msg
  )
}
```

## `gtkApp` — the standard app host

`gtkApp` is the recommended entry point for most GTK applications in AIVI. You give it a configuration record, and it handles init, startup, window creation, event ingestion, reconciliation, and the command/subscription flow described in [GTK App Architecture](/stdlib/ui/app_architecture).

```aivi
gtkApp : {
  id:     Text,
  title:  Text,
  size:   (Int, Int),
  model:  s,
  onStart: AppId -> WindowId -> Effect GtkError Unit,
  subscriptions: s -> List (Subscription msg),
  view:   s -> GtkNode,
  toMsg:  GtkSignalEvent -> Option msg,
  update: msg -> s -> Effect GtkError (AppStep s msg)
} -> Effect GtkError Unit
```

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

```aivi
reconcileNode : WidgetId -> GtkNode -> Effect GtkError WidgetId
```

It returns the root `WidgetId`: the same id when the root widget was patched in place, or a new id if the root widget type had to be rebuilt.

Properties are patched, CSS classes are diffed, signal handlers are reconnected when bindings change, and children are reconciled positionally.

### Full example

```aivi
Msg = TitleChanged Text | BodyChanged Text | Save

editorView : { title: Text, body: Text } -> GtkNode
editorView = state => ~<gtk>
  <GtkBox orientation="vertical" spacing="8">
    <GtkEntry id="titleInput" placeholderText="Title" onInput={ TitleChanged } />
    <GtkEntry id="bodyInput" placeholderText="Body" onInput={ BodyChanged } />
    <GtkButton label="Save" onClick={ Msg.Save } />
  </GtkBox>
</gtk>

update : Msg -> { title: Text, body: Text } -> Effect GtkError (AppStep { title: Text, body: Text } Msg)
update = msg => state =>
  msg match
    | TitleChanged txt =>
        pure { model: state <| { title: txt }, commands: [] }
    | BodyChanged txt =>
        pure { model: state <| { body: txt }, commands: [] }
    | Save =>
        do Effect {
          _ <- saveNote state
          pure { model: state, commands: [] }
        }

main : Effect GtkError Unit
main = gtkApp {
  id: "com.example.notepad",
  title: "Notepad",
  size: (640, 480),
  model: { title: "", body: "" },
  onStart: _ _ => pure Unit,
  subscriptions: noSubscriptions,
  view: editorView,
  toMsg: auto,
  update: update
}
```

## Diagnostics

- `E1612`: invalid `props` shape (must be a compile-time record literal)
- `E1613`: non-literal `props` field value
- `E1614`: invalid signal binding (`onClick`, `onInput`, `onActivate`, `onToggle`, `onValueChanged`, `onFocusIn`, `onFocusOut`, and `<signal ... on={...}>` require compile-time values)
- `E1615`: invalid `<each>` usage (requires `items={...}`, `as={...}`, and exactly one child template node)

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
