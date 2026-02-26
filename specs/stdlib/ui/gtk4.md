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
| `signalPoll` | `gtk4.signalPoll` |
| `signalEmit` | `gtk4.signalEmit` |
| `osOpenUri` | `gtk4.osOpenUri` |
| `osShowInFileManager` | `gtk4.osShowInFileManager` |
| `osSetBadgeCount` | `gtk4.osSetBadgeCount` |
| `osThemePreference` | `gtk4.osThemePreference` |

## Example

<<< ../../snippets/from_md/stdlib/ui/gtk4/example.aivi{aivi}

## GTK XML sigil (`~<gtk>...</gtk>`)

`aivi.ui.gtk4` also exposes typed constructors used by the GTK XML sigil:

- `GtkNode = GtkElement Text (List GtkAttr) (List GtkNode) | GtkTextNode Text`
- `GtkAttr = GtkAttribute Text Text`
- helpers: `gtkElement`, `gtkTextNode`, `gtkAttr`
- `GtkSignalEvent = GtkSignalEvent WidgetId Text Text Text`

The parser lowers `~<gtk>...</gtk>` into those constructors.
Instantiate the resulting node tree with `buildFromNode`.
`buildFromNode` accepts `<object>`, `<interface>`, or `<template>` roots.
For `<interface>`/`<template>`, the first nested `<object>` becomes the instantiated root.
Object references via `ref`/`idref` are resolved against `id` attributes.
`<child type="overlay">` and `<child type="controller">` are supported for overlay/controller wiring.
Header-bar child placement is supported via `<child type="title">` and `<child type="end">` (`start` is the default).
Signal sugar is supported and lowered to typed signal attrs:

- `<object ... onClick={ Msg.Save } />` -> `signal:clicked`
- `<object ... onInput={ Msg.Changed } />` -> `signal:changed`
- `<signal name="clicked" on={ Msg.Save } />` -> same binding path

Signal handler values must be compile-time expressions (for example constructor-like tags such as `Msg.Save`), not runtime lambdas.

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

Lowering shape:

- `Row [gtkAttrs...] [gtkChildren...]`
- `Ui.Row [gtkAttrs...] [gtkChildren...]`

Lowercase GTK tags continue to lower to `gtkElement`.

`props={ { ... } }` sugar on any tag expands to normalized GTK properties:

- `marginTop` becomes `prop:margin-top`
- `spacing` stays `prop:spacing`

In v0.1, `props` must be a compile-time record literal; dynamic `props={expr}` is a diagnostic.

`signalPoll : Unit -> Effect GtkError (Option GtkSignalEvent)` reads queued runtime signal events.
`signalEmit` is available for synthetic/manual event injection (useful in tests and mock-driven flows).

### Example: builder + property sugar

```aivi
uiNode : GtkNode
uiNode =
  ~<gtk>
    <object class="GtkBox" props={ { orientation: "vertical", spacing: 12, marginTop: 16 } }>
      <child>
        <object class="GtkLabel">
          <property name="label">Settings</property>
        </object>
      </child>
    </object>
  </gtk>
```

### Example: signal sugar (recommended style)

```aivi
Msg = Save | NameChanged

formNode : GtkNode
formNode =
  ~<gtk>
    <object class="GtkBox" props={ { orientation: "vertical", spacing: 8 } }>
      <child>
        <object class="GtkEntry" onInput={ Msg.NameChanged } />
      </child>
      <child>
        <object class="GtkButton" onClick={ Msg.Save }>
          <property name="label">Save</property>
        </object>
      </child>
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
        <child>
          <object class="GtkLabel">
            <property name="label">{ item }</property>
          </object>
        </child>
      </each>
    </object>
  </gtk>
```

### Example: consuming queued signal events

```aivi
nextMsg : Effect GtkError (Option Text)
nextMsg = effect {
  eventOpt <- signalPoll {}
  eventOpt match
    | None => yield None
    | Some (GtkSignalEvent _ signal handler payload) =>
        yield Some "{ signal }|{ handler }|{ payload }"
}
```

### Diagnostics

- `E1612`: invalid `props` shape (must be compile-time record literal).
- `E1613`: non-literal `props` field value.
- `E1614`: invalid signal binding (`onClick`/`onInput`/`<signal ... on={...}>` requires compile-time values).
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
