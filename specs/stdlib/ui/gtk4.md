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
- `~<gtk>...</gtk>` with a root `GtkWindow` or `GtkApplicationWindow`
- callback attrs such as `onClick={handler}` or `onClick={eventHandle}`
- structural bindings such as `<show>` and `<each key={...}>`
- `signalStream`, `signalPoll`, and `signalEmit` only for lower-level integrations and tests

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

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="12" marginTop="12" marginStart="12">
    <GtkLabel label={title} />
    <GtkButton label="Increment" onClick={increment} />
  </GtkBox>
</gtk>

main = do Effect {
  _ <- init Unit
  appId <- appNew "com.example.counter"
  win <- windowNew appId "Counter" 640 480
  root <- buildFromNode view
  _ <- windowSetChild win root
  _ <- windowPresent win
  appRun appId
}
```

There is no required `Model -> Msg -> update` host. Signals are the source of truth, `buildFromNode` mounts the tree, and later signal writes mutate the live widgets directly.

## Core signal-first surface

The core surface is:

- reactive values: `signal`, `get`, `set`, `update`, `derive`, `combineAll`, `watch`, `on`, `batch`, `peek`
- effect handles: `do Event { ... }` with `result`, `error`, `done`, and `running`
- GTK binding surface: `~<gtk>...</gtk>`, callback attributes, `<show>`, `<each>`, and widget/window helpers
- low-level escape hatches: `buildFromNode`, `buildWithIds`, `signalStream`, `signalPoll`, `signalEmit`

Useful lower-level runtime functions still include:

| AIVI function | Native target |
| --- | --- |
| `init` | `gtk4.init` |
| `appNew` | `gtk4.appNew` |
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

GTK sigils also support function-call tags for local lowerCamel helpers. A simple uppercase self-closing tag with positional arguments lowers to the same helper with a lowercased first letter:

<<< ../../snippets/from_md/stdlib/ui/gtk4/block_01.aivi{aivi}

Function-call tags:

- only apply to simple non-`Gtk*`/`Adw*`/`Gsk*` tags
- use positional arguments instead of attributes
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

The runtime directly covers the standard signal-first widget story:

- root windows: `GtkWindow`, `GtkApplicationWindow`
- layout containers: `GtkBox`, `GtkGrid`, `GtkOverlay`, `GtkScrolledWindow`, `GtkStack`, `GtkRevealer`, `GtkPaned`, `GtkHeaderBar`, `AdwHeaderBar`, `AdwClamp`
- interactive widgets: `GtkButton`, `GtkCheckButton`, `GtkToggleButton`, `GtkSwitch`, `GtkEntry`, `GtkPasswordEntry`, `GtkSearchEntry`, `GtkTextView`, `GtkScale`, `GtkSpinButton`, `GtkDropDown`, `GtkMenuButton`, `GtkSpinner`, `GtkProgressBar`
- display widgets: `GtkLabel`, `GtkImage`, `GtkPicture`, `GtkDrawingArea`, `GtkSeparator`, `GtkListBox`
- event controllers: `GtkGestureClick`, `GtkEventControllerMotion` via raw `enter`/`leave` signals, keyboard capture via `onKeyPress={...}`

Additional `Adw*` classes can be created dynamically when their GType is available. Container-specific child-slot rules remain a thin handwritten layer even when widget metadata comes from GIR/GObject reflection.

## Low-level runtime helpers

The signal-first host is the normal path, but lower-level tools still matter for libraries, tests, and embedding:

- `buildFromNode` builds a GTK subtree from `<object>`, `<interface>`, or `<template>`
- `buildWithIds` returns `{ root, widgets }` for named-widget lookup
- `signalPoll` reads one queued `GtkSignalEvent`
- `signalStream` exposes the raw GTK event stream as a `Recv GtkSignalEvent`
- `signalEmit` injects synthetic events for tests or mocks

For standard apps, bound callbacks and event handles are the normal path. `signalStream` and `signalPoll` are lower-level escape hatches.

## Windows, dialogs, and application infrastructure

`GtkWindow` and `GtkApplicationWindow` are valid root nodes in signal-first GTK trees. They should appear at the root of a mounted app tree rather than as nested children.

Dialogs and menus remain programmatic surfaces:

| Type | Notes |
| --- | --- |
| `GtkAlertDialog` | asynchronous dialog API object; create and present programmatically or from an `Event`/callback |
| `GtkDialog`, `GtkMessageDialog` | legacy dialog APIs; prefer `GtkAlertDialog` or libadwaita dialogs |
| `GtkPopover`, `GtkPopoverMenu` | transient popup/menu surfaces; still created and presented programmatically |
| `GtkFileDialog` | asynchronous file chooser object; create and present programmatically |
| `GSimpleAction` / `GMenu` | programmatic action and menu infrastructure |

GIO actions and menus are still wired up programmatically. In the signal-first model, do that from startup/mount-time effects or other library setup code rather than a reducer host hook.

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

## Notable non-widget-tree surfaces

Some GTK/libadwaita types are still relevant but are not direct widget-tree nodes:

- animation objects such as `AdwTimedAnimation` and `AdwSpringAnimation`
- list/model infrastructure such as `GtkSelectionModel`, `GtkFilterListModel`, and `GtkSortListModel`
- printing infrastructure such as `GtkPrintOperation`
- singleton/builder infrastructure such as `GtkSettings`, `GtkStyleContext`, and `GtkBuilder`

Use these programmatically when needed; they are not the primary signal-first app surface.
