# `aivi.ui.gtk4`
## GTK & libadwaita System Manual

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the native desktop UI system for AIVI. It mounts a root GTK tree once, binds widget state directly to `Signal` values, routes widget callbacks to runtime functions or `Event` handles, and performs structure-aware patching only for mounted `<show>` and `<each>` bindings.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

This is the single canonical guide for AIVI's GTK story. Older split pages such as "Native GTK & libadwaita Apps" and "Signal-First GTK Architecture" have been folded into this manual.

## How UI effects fit into AIVI

GTK runtime operations are ordinary effects and resources in AIVI:

- root widget mounting and window presentation -> `ui.window`
- `signalPoll`, `signalStream`, `signalEmit` -> `ui.signal`
- first-class `Signal` and `Event` values -> the reactive runtime layer
- clipboard helpers -> `ui.clipboard`
- desktop notification helpers -> `ui.notification`

These operations live in AIVI's ordinary effect system. They do not create a separate “special UI language”.

## Start here

Most GTK apps only need this smaller subset:

- `Signal` values for authoritative UI state
- `derive` and `combineAll` for derived reactive state
- `~<gtk>...</gtk>` with a root `GtkWindow`, `GtkApplicationWindow`, `AdwWindow`, or `AdwApplicationWindow`
- `runGtkApp` for the normal signal-first app entry point
- callback attrs such as `onClick={handler}` or `onClick={eventHandle}`
- structural bindings such as `<show>` and `<each key={...}>`
- `mountAppWindow`, `signalStream`, `signalPoll`, and `signalEmit` only for lower-level integrations and tests

## Choosing the right entry point

| If you need to... | Start with... |
| --- | --- |
| build a normal single-window desktop app | a root `~<gtk>` tree with signals |
| describe a widget tree in the most readable way | `~<gtk>...</gtk>` with shorthand tags |
| derive reactive UI data | `derive` and `combineAll` |
| run effectful UI work from a callback | `Event` handles or direct runtime functions |
| consume the raw GTK event queue in a library or test | `signalStream`, `signalPoll`, `signalEmit` |
| grab widget ids for direct low-level access | `buildWithIds` |

## Minimal signal-first example

```aivi
use aivi.reactive
use aivi.ui.gtk4

state = signal { count: 0 }
title = derive state (s => "Count {s.count}")
increment = _ => update state (patch { count: _ + 1 })

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="8">
    <GtkLabel label={title} />
    <GtkButton label="Increment" onClick={increment} />
  </GtkBox>
</gtk>
```

## Mounting and running a GTK app

Standard apps still use ordinary GTK lifecycle effects. The signal-first part is the mounted tree, not a separate host loop:

```aivi
use aivi
use aivi.reactive
use aivi.ui.gtk4

state = signal { count: 0 }
title = derive state (s => "Count {s.count}")
increment = _ => update state (patch { count: _ + 1 })

root = ~<gtk>
  <GtkApplicationWindow title="Counter" defaultWidth={640} defaultHeight={480}>
    <GtkBox orientation="vertical" spacing="12" marginTop="12" marginStart="12">
      <GtkLabel label={title} />
      <GtkButton label="Increment" onClick={increment} />
    </GtkBox>
  </GtkApplicationWindow>
</gtk>

main = runGtkApp {
  appId: "com.example.counter"
  root: root
  onStart: pure Unit
}
```

There is no required `Model -> Msg -> update` host. Signals are the source of truth, the root GTK tree is mounted once, and later signal writes mutate the live widgets directly.

## Core signal-first surface

The core surface is:

- reactive values: `signal`, `get`, `set`, `update`, `derive`, `combineAll`, `watch`, `on`, `batch`, `peek`
- effect handles: `do Event { ... }` with `result`, `error`, `done`, and `running`
- GTK binding surface: `~<gtk>...</gtk>`, callback attributes, `<show>`, `<each>`, and widget/window helpers
- low-level escape hatches: `mountAppWindow`, `buildFromNode`, `buildWithIds`, `signalStream`, `signalPoll`, `signalEmit`

Useful lower-level runtime functions still include:

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
| `mountAppWindow` | `gtk4.mountAppWindow` |
| `windowNew` | `gtk4.windowNew` |
| `windowSetTitle` | `gtk4.windowSetTitle` |
| `windowSetChild` | `gtk4.windowSetChild` |
| `windowPresent` | `gtk4.windowPresent` |
| `buildFromNode` | `gtk4.buildFromNode` |
| `buildWithIds` | `gtk4.buildWithIds` |
| `signalPoll` | `gtk4.signalPoll` |
| `signalStream` | `gtk4.signalStream` |
| `signalEmit` | `gtk4.signalEmit` |
| `widgetSetCss` | `gtk4.widgetSetCss` |
| `appSetCss` | `gtk4.appSetCss` |
| `menuButtonSetMenuModel` | `gtk4.menuButtonSetMenuModel` |
| `drawAreaQueueDraw` | `gtk4.drawAreaQueueDraw` |

## GTK XML sigil (`~<gtk>...</gtk>`)

The GTK sigil is the most approachable way to describe widget trees. If you know JSX, SwiftUI-style builders, or XML UI files, the idea is similar: you write a tree-shaped description, and the runtime turns it into mounted GTK objects with live bindings.

The sigil lowers into a binding-preserving GTK IR. Conceptually it must retain:

- static props
- bound props
- static and bound text
- event handlers as runtime functions or `Event` handles
- structural binders such as `<show>` and `<each>`
- ids, refs, and child-slot metadata

The public promise is semantic, not constructor-name-specific: the runtime must preserve enough information to mutate the exact GTK target that depends on a changed signal.

### Shorthand widget tags (preferred)

Tags starting with `Gtk`, `Adw`, or `Gsk` are shorthand for `<object class="...">`.

```aivi
// Shorthand (preferred)
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Save" onClick={saveEvent} />
  </GtkBox>
</gtk>

// Equivalent verbose form
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="GtkLabel" props={{ label: title }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={saveEvent} />
  </object>
</gtk>
```

Attributes on shorthand tags become properties automatically, except for pass-through attributes such as `id` and `ref`.

| Attribute | Lowering |
| --- | --- |
| `label="Save"` | static property |
| `marginTop="12"` | static property |
| `title={title}` | bound property |
| `id="saveBtn"` | widget id |
| `ref="btnRef"` | widget reference |
| `onClick={...}` | event binding |
| `onInput={...}` | event binding |
| `onKeyPress={...}` | keyboard event binding |

### Which syntax to use

| Syntax style | Reach for it when... |
| --- | --- |
| shorthand tags such as `<GtkButton ... />` | day-to-day app code; this is the most readable default |
| verbose `<object class="...">` form | you need builder-style structure or want to mirror GTK builder docs closely |
| explicit `<signal ... />` tags | the signal name is clearer written explicitly or does not map neatly to sugar |

### Component and function-call tags

Uppercase or dotted non-`Gtk*`/`Adw*` tags are component calls:

- `<ProjectRow ... />`
- `<Ui.ProjectRow ... />`

Component tags use record-based lowering: attributes become record fields and children become a `children` field. Signal sugar and `props` normalization do not apply there because the component function owns its own API.

GTK sigils also support function-call tags for local lowerCamel helpers. A simple uppercase self-closing tag lowers to the same helper with a lowercased first letter:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_01.aivi{aivi}

When the self-closing helper tag has no attributes, children, or positional arguments, the sigil passes `Unit` automatically. For example, `<DetailsPane />` lowers like `detailsPane Unit`.

Function-call tags:

- only apply to simple non-`Gtk*`/`Adw*`/`Gsk*` tags
- use positional arguments instead of attributes, or `Unit` when there are none
- must be self-closing
- do not participate in component record lowering

## Callback binding contract

Signal sugar works on both shorthand and verbose tags:

- `onClick={handler}` -> `signal:clicked`
- `onInput={handler}` -> `signal:changed`
- `onActivate={handler}` -> `signal:activate`
- `onKeyPress={handler}` -> `signal:key-pressed`
- `onToggle={handler}` -> `signal:toggled`
- `onValueChanged={handler}` -> `signal:value-changed`
- `onFocusIn={handler}` -> `signal:focus-enter`
- `onFocusOut={handler}` -> `signal:focus-leave`
- `<signal name="clicked" on={handler} />` -> the same binding path
- `GtkEventControllerMotion` uses explicit `<signal name="enter" ... />` / `<signal name="leave" ... />` children rather than sugar attrs

Callback values may be either runtime functions or `Event` handles. For the common sugar attrs, the function receives the useful GTK payload when one exists:

- `onInput` -> current `Text`
- `onKeyPress` -> `GtkKeyPressed WidgetId Text Text Text` (pattern match the `key` field in the callback)
- `onToggle` -> current `Bool`
- `onValueChanged` -> current `Float`
- click/focus-style signals -> a unit-like or widget event payload, depending on the underlying signal
- raw motion-controller `enter` / `leave` bindings can ignore the payload or match them as `GtkUnknownSignal ... "enter" ...` and `GtkUnknownSignal ... "leave" ...` in lower-level event consumers

Valid public shapes therefore include:

```aivi
<GtkButton onClick={_ => update state (patch { count: _ + 1 })} />
<GtkEntry onInput={txt => set query txt} />
<GtkBox onKeyPress={event =>
  event match
    | GtkKeyPressed _ _ key _ => handleKey key
    | _ => pure Unit
} />
<GtkSwitch onToggle={active => set enabled active} />
<GtkButton onClick={saveEvent} />
<GtkBox>
  <child type="controller">
    <GtkEventControllerMotion>
      <signal name="enter" on={_ => set hovered True} />
      <signal name="leave" on={_ => set hovered False} />
    </GtkEventControllerMotion>
  </child>
</GtkBox>
```

`GtkSignalEvent` remains the low-level event ADT for queue-based APIs and tests:

Container-specific `<child type="...">` slots follow the underlying GTK/libadwaita widget API. For example, `AdwToolbarView` uses `<child type="top">` and `<child type="bottom">` for toolbar bars, while an untyped direct child (or `<property name="child">`) becomes the main content widget. When a surface is not a plain widget child, nest the helper object under `<property name="...">`; this is the normal escape hatch for object-valued GTK properties such as `model`, `factory`, `adjustment`, `stream`, and `layout-manager`.

```aivi
~<gtk>
  <GtkListView>
    <property name="model">
      <GtkNoSelection>
        <property name="model">
          <GtkStringList id="items" />
        </property>
      </GtkNoSelection>
    </property>
    <property name="factory">
      <GtkSignalListItemFactory />
    </property>
  </GtkListView>
</gtk>
```

```aivi
GtkSignalEvent =
  | GtkClicked       WidgetId Text
  | GtkInputChanged  WidgetId Text Text
  | GtkActivated     WidgetId Text
  | GtkToggled       WidgetId Text Bool
  | GtkValueChanged  WidgetId Text Float
  | GtkKeyPressed    WidgetId Text Text Text
  | GtkFocusIn       WidgetId Text
  | GtkFocusOut      WidgetId Text
  | GtkUnknownSignal WidgetId Text Text Text Text
```

The second field is the widget's `id="..."` name, or `""` when no `id` is set. That is why lower-level integrations often match GTK events by widget name instead of comparing integer ids.

## Structural bindings

Dynamic child positions are mounted structural bindings, not flattened rerenders:

```aivi
<show when={visible}>
  <DetailsPane />
</show>

<each items={items} as={item} key={item => item.id}>
  <ProjectRow project={item} />
</each>
```

The public contract is:

- `<show>` mounts or disposes one child scope as its guard signal changes
- `<each>` preserves one mounted child scope per key
- keyed children move instead of being recreated when possible
- inserts and removals happen through the owning GTK container, not a generic VDOM diff

## Runtime coverage

The runtime now combines hand-written fast paths for the common signal-first story with GIR-derived metadata for the broader GTK4/libadwaita surface.

The public contract is:

- any indexed `Gtk*` / `Adw*` class that is valid in the surrounding GTK API may be instantiated from `~<gtk>`, `buildFromNode`, or `buildWithIds`
- writable scalar properties still use the existing hand-tuned setters where needed, then fall back to metadata-driven generic property application
- object-valued properties can be expressed explicitly with nested `<property name="..."> <object .../> </property>` nodes
- common single-child containers fall back to common pointer properties such as `child` and `content` when they do not need a custom slot API
- container-specific `<child type="...">` attachment remains a thin handwritten layer for widget families that require specialized add/remove calls
- typed signal payloads remain curated for the common AIVI event sugar, while other indexed signals may still connect and surface through `GtkUnknownSignal`

That means the runtime is no longer limited to a small allowlist of hand-wrapped classes. In addition to the standard `GtkBox` / `GtkButton` / `GtkEntry` flow, metadata-driven construction now covers broader widget families such as `GtkListView`, `GtkColumnView`, `GtkGridView`, `GtkPopover`, `GtkPopoverMenu`, `GtkPopoverMenuBar`, `GtkVideo`, and `GtkMediaControls`, plus helper-object graphs such as `GtkStringList`, `GtkAdjustment`, `GtkSelectionModel` implementations, `GtkFilterListModel`, `GtkSortListModel`, `GtkSignalListItemFactory`, `GtkBuilderListItemFactory`, and layout-manager objects when they are wired through the appropriate property.

## Low-level runtime helpers

The signal-first host is the normal path, but lower-level tools still matter for libraries, tests, and embedding:

- `mountAppWindow` mounts a list of app roots, returns the primary `WindowId`, and keeps every mounted root live from signal writes
- `buildFromNode` builds a GTK subtree from `<object>`, `<interface>`, or `<template>`
- `buildWithIds` returns `{ root, widgets }` for named-widget lookup
- `signalPoll` reads one queued `GtkSignalEvent`
- `signalStream` exposes the raw GTK event stream as a `Recv GtkSignalEvent`
- `signalEmit` injects synthetic events for tests or mocks

For standard apps, bound callbacks and event handles are the normal path. `signalStream` and `signalPoll` are lower-level escape hatches.

## Windows, dialogs, and application infrastructure

Any GTK/libadwaita class that is a `GtkWindow` subclass is a valid primary root node in a signal-first GTK tree. In practice that usually means `GtkWindow`, `GtkApplicationWindow`, `AdwWindow`, `AdwApplicationWindow`, and other concrete window/dialog subclasses. They should appear at the root of a mounted app tree rather than as nested children. Use `runGtkApp` for the common case, or `mountAppWindow` when you need manual access to the mounted `WindowId` before calling `appRun` or when you need to mount extra live roots such as persistent libadwaita dialogs.

`mountAppWindow : AppId -> List GtkNode -> Effect GtkError WindowId` has these rules:

- the list must contain at least one root
- the first entry is the primary app window root and is the returned `WindowId`
- later entries mount as additional live roots under the same app/runtime
- `Adw*Dialog` extra roots may omit `presentFor` / `transientFor`; they will default to the primary window

```aivi
windowRoot = ~<gtk>
  <AdwApplicationWindow title="Mailfox">
    <GtkBox />
  </AdwApplicationWindow>
</gtk>

settingsDialog = ~<gtk>
  <AdwPreferencesDialog id="settings-dialog" open={settingsOpen}>
    <AdwPreferencesPage title="General" />
  </AdwPreferencesDialog>
</gtk>

main = do Effect {
  _   <- init Unit
  app <- appNew "com.example.mailfox"
  win <- mountAppWindow app [windowRoot, settingsDialog]
  _   <- windowPresent win
  appRun app
}
```

Some special-lifecycle APIs still need specialized handling:

| Type | Notes |
| --- | --- |
| `GtkAlertDialog` | asynchronous dialog API object; create and present programmatically or from an `Event`/callback |
| `GtkDialog`, `GtkMessageDialog` | legacy top-level dialog window APIs; mount them as root windows only if needed, but prefer `GtkAlertDialog` or libadwaita dialogs |
| `GtkFileDialog` | asynchronous file chooser object; create and present programmatically |
| `GSimpleAction` / `GMenu` | programmatic action and menu infrastructure |

`GtkPopover` / `GtkPopoverMenu` / `GtkPopoverMenuBar` and the newer list/media widgets are ordinary tree nodes now; only the truly async or application-lifecycle APIs above remain outside the declarative tree surface.

GIO actions and menus are still wired up programmatically. In the signal-first model, do that from startup/mount-time effects or other library setup code rather than a reducer host hook.

When a `GSimpleAction` created with `actionNew` is activated, AIVI pushes a raw runtime event onto `signalStream`. Match it as:

```aivi
handleRuntimeEvent = event => event match
  | GtkUnknownSignal _ _ "action" actionName _ => ...
  | _                                          => pure Unit
```

For application menus, create the action with its bare name (for example `"settings-ai"`), add it with `appAddAction`, and reference it from menu items with the usual `"app.settings-ai"` detailed action string.

## Custom drawing and redraw

For drawing areas or custom stateful widgets, the pattern is:

1. represent UI state as one or more signals
2. update those signals from GTK callbacks or `Event` results
3. queue imperative redraws only for the widgets that need them
4. keep watchers and redraw hooks tied to the mounted scope

```aivi
points = signal []

addPoint = point => batch (_ =>
  do Effect {
    update points (_ ++ [point])
    drawAreaQueueDraw canvas
    pure Unit
  }
)
```

## Diagnostics

- `E1612`: invalid `props` shape (must be a record literal the sigil can inspect)
- `E1613`: unsupported `props` field value in a position that requires static inspection
- `E1614`: invalid signal binding (`onClick`, `onInput`, `onActivate`, `onKeyPress`, `onToggle`, `onValueChanged`, `onFocusIn`, `onFocusOut`, and `<signal ... on={...}>` require a function or an `Event` handle with a compatible payload shape)
- `E1615`: invalid `<each>` usage (requires `items={...}`, `as={...}`, and exactly one child template node; keyed iteration is the recommended contract)
- `E1616`: bare `<child>` without a `type` attribute (nest `<object>` elements directly inside the parent instead)
- `E1617`: invalid GTK function-call tag usage (function-call sugar must use positional arguments on a self-closing tag and cannot mix with attributes)
- runtime binding failures report the widget id/class, optional `id="..."` name, the failing binding, and the known supported signals for that class

## Special and managed surfaces

Some GTK/libadwaita types are still relevant but need extra care:

- helper-object graphs such as `GtkSelectionModel`, `GtkFilterListModel`, `GtkSortListModel`, `GtkStringList`, `GtkAdjustment`, `GtkSignalListItemFactory`, `GtkBuilderListItemFactory`, and layout-manager objects may now appear as nested object-valued `<property>` children
- animation objects such as `AdwTimedAnimation` and `AdwSpringAnimation` remain programmatic helpers rather than common tree roots
- printing infrastructure such as `GtkPrintOperation`, singleton/builder infrastructure such as `GtkSettings`, `GtkStyleContext`, and `GtkBuilder`, and lifecycle objects such as `GtkApplication`, `GtkAlertDialog`, and `GtkFileDialog` remain outside the normal declarative tree surface
- GTK-managed/internal types such as `GtkListItem`, `GtkColumnViewColumn`, `GtkColumnViewRow`, `GtkColumnViewCell`, `GtkColumnViewSorter`, shortcut helper objects, and `GtkMediaStream` still require their owning GTK API rather than direct tree construction

Use these through the appropriate property, helper function, or programmatic GTK integration point rather than treating every type as a drop-in root widget.
