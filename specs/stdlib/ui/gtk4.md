# `aivi.ui.gtk4`
## Writing Native Apps with GTK & libadwaita

<!-- quick-info: {"kind":"module","name":"aivi.ui.gtk4"} -->
`aivi.ui.gtk4` is the native desktop UI system for AIVI. It mounts a live GTK tree, binds widget props and structure directly to `Signal` values, routes callbacks to runtime functions or `EventHandle` values, and updates only the mounted parts of the UI that depend on changed reactive data.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.gtk4</div>

This page is a chaptered guide. Read it top to bottom the first time, then use the later chapters as reference.

## Chapter 1: Mental model

AIVI native apps are built from five pieces that fit together directly:

- `Signal` values hold live app state.
- `signal ->> ...` derives live read-only state.
- `signal <<- ...` writes live state.
- `~<gtk>...</gtk>` describes the mounted GTK tree.
- callbacks and `do Event { ... }` values connect user input to state changes and effects.

GTK runtime APIs are regular AIVI values and effects. The day-to-day surface is small:

- state: `signal`, `get`, `peek`, `->>`, `<<-`, `combineAll`
- view: `~<gtk>...</gtk>`, shorthand widget tags, helper tags, `<show>`, `<each>`
- input: `onClick`, `onInput`, `onToggle`, `onSelect`, `onKeyPress`, and raw `<signal ... />`
- effectful actions: `do Event { ... }`
- app startup: `runGtkApp`
- lower-level escape hatches: `mountAppWindow`, `buildFromNode`, `buildWithIds`, `reconcileNode`, `signalStream`, `signalPoll`, `signalEmit`

A good default is:

1. keep authoritative state in signals,
2. derive display-only data with `->>`,
3. render with the GTK sigil,
4. update signals from callbacks,
5. use `do Event { ... }` when a callback should own shared effect state.

### Choosing the starting point

| If you need to... | Start with... |
| --- | --- |
| build a normal single-window app | `runGtkApp` plus a root `~<gtk>` tree |
| keep UI state live | `signal`, `->>`, `<<-` |
| wire user input from common widgets | callback sugar such as `onClick` and `onInput` |
| run one shared effectful action from several widgets | `do Event { ... }` and pass the handle to the widgets |
| show or repeat dynamic child content | `<show>` and `<each key={...}>` |
| debug or test below the sugar layer | `buildWithIds`, `signalStream`, `signalPoll`, `signalEmit` |

## Chapter 2: Your first window

A minimal counter already shows the core style:

```aivi
use aivi.reactive
use aivi.ui.gtk4

state = signal { count: 0 }
title = state ->> .count ->> (n => "Count {n}")

increment = _ => state <<- { count: _ + 1 }

root = ~<gtk>
  <GtkApplicationWindow title="Counter" defaultWidth={640} defaultHeight={480}>
    <GtkBox
      orientation="vertical"
      spacing="12"
      marginTop="12"
      marginBottom="12"
      marginStart="12"
      marginEnd="12"
    >
      <GtkLabel label={title} />
      <GtkButton id="incrementButton" label="Increment" onClick={increment} />
    </GtkBox>
  </GtkApplicationWindow>
</gtk>

main = runGtkApp {
  appId: "com.example.counter"
  root: root
  onStart: pure Unit
}
```

What to notice:

- `state` is the source of truth.
- `title` is another signal, created with `->>`.
- the button callback writes the signal with `<<-`.
- the label binds directly to a signal.
- `id="incrementButton"` gives the widget a stable debug name for MCP and lower-level event matching.

## Chapter 3: State, derivation, and patching

Use ordinary data for defaults and pure transforms, then move into signals when the value must stay live.

```aivi
baseState = {
  title: "Mailfox",
  draft: { subject: "", body: "" },
  folders: [
    { id: "inbox", name: "Inbox" },
    { id: "archive", name: "Archive" }
  ]
}

state = signal (baseState <| { title: "Mailfox Dev" })

windowTitle = state ->> .title
folderNames = state ->> (.folders |> map .name)

renameWindow = text => state <<- { title: text }
resetDraft = _ => state <<- { draft: { subject: "", body: "" } }
```

Use the operators with this rule of thumb:

- `value <| { ... }` updates ordinary immutable data and returns a new value.
- `signal <<- value` replaces the signal's value.
- `signal <<- fn` updates from the previous value.
- `signal <<- { ... }` applies patch semantics to the current record value.
- `signal ->> rhs` is shorthand for deriving a signal whose mapper reads like `value |> rhs`.

### Deriving from more than one live source

When a derived value depends on several signals, use `combineAll`:

```aivi
title = signal ""
saveBusy = signal False

canSave = combineAll (title, saveBusy) ((currentTitle, busy) =>
  currentTitle != "" && !busy
)
```

Use `->>` for one signal, `combineAll` for several.

## Chapter 4: Writing GTK trees with sugar

The GTK sigil is the main authoring surface:

```aivi
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Save" onClick={saveDraft} />
  </GtkBox>
</gtk>
```

### 4.1 Shorthand widget tags

Tags beginning with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="...">`.

```aivi
// Preferred shorthand
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Save" onClick={saveDraft} />
  </GtkBox>
</gtk>

// Equivalent verbose form
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="GtkLabel" props={{ label: title }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={saveDraft} />
  </object>
</gtk>
```

Attributes on shorthand tags lower automatically:

| Attribute | Meaning |
| --- | --- |
| `label="Save"` | static property |
| `title={windowTitle}` | bound property |
| `id="saveButton"` | widget name for inspection and event matching |
| `ref="saveRef"` | widget reference |
| `onClick={...}` | event binding |
| `onInput={...}` | event binding |
| `onActivate={...}` | activate binding |
| `onToggle={...}` | toggle binding |
| `onSelect={...}` | selection binding |
| `onClosed={...}` | dialog close binding |
| `onKeyPress={...}` | keyboard binding |
| `onValueChanged={...}` | range binding |
| `onFocusIn={...}` | focus-enter binding |
| `onFocusOut={...}` | focus-leave binding |
| `onShowSidebarChanged={...}` | overlay split-view sidebar binding |

### 4.2 Component tags and function-call tags

Uppercase or dotted non-`Gtk*` / `Adw*` / `Gsk*` tags are component calls. Their attributes lower to a record-shaped argument.

```aivi
<ProjectRow row={row} selected={isSelected} />
<Mail.ProjectRow row={row} selected={isSelected} />
```

Function-call tags are the lighter helper form for simple self-closing tags with positional arguments. A simple uppercase helper tag lowers to the same helper with a lowercased first letter.

```aivi
// Equivalent to: { navRailNode currentSection "sidebar" }
~<gtk>
  <NavRailNode currentSection "sidebar" />
</gtk>
```

Function-call tags:

- only apply to simple non-widget tags,
- use positional arguments instead of attributes,
- must stay self-closing,
- pass `Unit` automatically when there are no positional arguments, so `<DetailsPane />` lowers like `detailsPane Unit`.

### 4.3 Object-valued properties and child slots

Use nested `<property name="...">` when GTK expects another object rather than plain text or numbers.

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

Use `<child type="...">` when the underlying GTK or libadwaita widget has named child slots.

## Chapter 5: Callback sugar in detail

Common GTK signals have direct sugar attributes. Use them when you want readable app code and typed payloads.

### 5.1 Which handler shape to use

Every sugared callback position accepts one of two authoring styles:

1. a function callback, when you need the payload,
2. an `EventHandle`, when the signal itself should trigger the handle's `.run` effect.

That leads to a simple rule:

- if you need the current text, bool, index, float, or key event, write a function,
- if you just want "run this action now", pass the event handle directly.

### 5.2 Sugar mapping and payloads

| Sugar | GTK signal | Function callback receives |
| --- | --- | --- |
| `onClick={...}` | `clicked` | `Unit` |
| `onInput={...}` | `changed` | `Text` |
| `onActivate={...}` | `activate` | `Unit` |
| `onToggle={...}` | `notify::active` for `GtkSwitch`, otherwise `toggled` | `Bool` |
| `onSelect={...}` | `notify::selected` for `GtkDropDown` | `Int` |
| `onClosed={...}` | `closed` for dialog widgets | `Unit` |
| `onValueChanged={...}` | `value-changed` | `Float` |
| `onFocusIn={...}` | `focus-enter` | `Unit` |
| `onFocusOut={...}` | `focus-leave` | `Unit` |
| `onShowSidebarChanged={...}` | `notify::show-sidebar` for `AdwOverlaySplitView` | `Bool` |
| `onKeyPress={...}` | `key-pressed` | `GtkKeyPressed WidgetId Text Text Text` |

For `onKeyPress`, the function callback receives the typed GTK event constructor, so pattern matching is usually the clearest style.

### 5.3 Direct payload callbacks

```aivi
use aivi.reactive
use aivi.ui.gtk4

form = signal { title: "", published: False }
themeIndex = signal 0

saveDraft : EventHandle GtkError Unit
saveDraft = do Event {
  persistDraft (get form)
  pure Unit
}

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="12">
    <GtkEntry
      id="titleInput"
      text={form ->> .title}
      onInput={text => form <<- { title: text }}
      onFocusOut={saveDraft}
    />
    <GtkSwitch
      id="publishSwitch"
      active={form ->> .published}
      onToggle={active => form <<- { published: active }}
    />
    <GtkDropDown
      id="themeSelect"
      strings="System\nLight\nDark"
      selected={themeIndex}
      onSelect={idx => themeIndex <<- idx}
    />
    <GtkButton id="saveButton" label="Save" onClick={saveDraft} />
  </GtkBox>
</gtk>
```

This example shows both styles together:

- `onInput`, `onToggle`, and `onSelect` use function callbacks because they need payloads,
- `onFocusOut` and `onClick` reuse the same `EventHandle` because the event itself is the trigger.

### 5.4 Keyboard callbacks

```aivi
use aivi.reactive
use aivi.ui.gtk4

sidebarOpen = signal True

refresh : EventHandle GtkError Unit
refresh = do Event {
  reloadMailbox
  pure Unit
}

handleKey = event => event match
  | GtkKeyPressed _ "mailWindow" "F5" _ => refresh.run
  | GtkKeyPressed _ "mailWindow" "Escape" _ => do Effect {
      sidebarOpen <<- False
      pure Unit
    }
  | _ => pure Unit
```

`GtkKeyPressed` carries four fields:

- numeric widget id,
- widget name from `id="..."`,
- key text,
- detail text.

Matching by the widget name is usually easier than comparing numeric ids.

### 5.5 Raw signal escape hatch

When there is no sugared attribute, or when you want the raw `GtkSignalEvent`, use explicit `<signal ... />` nodes.

`GtkEventControllerMotion` is the common example:

```aivi
hovered = signal False

view = ~<gtk>
  <GtkBox id="hoverTarget" orientation="vertical" spacing="4">
    <child type="controller">
      <GtkEventControllerMotion>
        <signal name="enter" on={_ => hovered <<- True} />
        <signal name="leave" on={_ => hovered <<- False} />
      </GtkEventControllerMotion>
    </child>
    <GtkLabel label="Hover target" />
  </GtkBox>
</gtk>
```

## Chapter 6: `do Event` in detail

`do Event { ... }` creates an `EventHandle E A`. The body uses the same readable effect style as `do Effect { ... }`, but the resulting value carries lifecycle state as signals.

### 6.1 The public shape

`EventHandle E A` has these fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `run` | `Effect E A` | Runs the underlying effect now. |
| `result` | `Signal (Option A)` | Last successful result, if any. |
| `error` | `Signal (Option E)` | Last failure, if any. |
| `done` | `Signal Bool` | `True` after the handle has completed. |
| `running` | `Signal Bool` | `True` while the handle is currently running. |

### 6.2 A full event-handle example

```aivi
use aivi.reactive
use aivi.ui.gtk4

draft = signal { title: "Inbox" }

saveDraft : EventHandle GtkError Text
saveDraft = do Event {
  persistDraft (get draft)
  pure "Saved"
}

saveLabel = saveDraft.running ->>
  | True  => "Saving..."
  | False => "Save"

saveFeedback = combineAll (saveDraft.result, saveDraft.error) ((result, error) =>
  error match
    | Some err => "Save failed: {err}"
    | None     =>
        result match
          | Some text => text
          | None      => ""
)

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="12">
    <GtkButton label={saveLabel} onClick={saveDraft} />
    <GtkLabel label={saveFeedback} />
  </GtkBox>
</gtk>
```

Because `result`, `error`, `done`, and `running` are signals, they participate in the same reactive graph as the rest of the app. Bind them directly to labels, sensitivity flags, spinners, and status views.

### 6.3 When to use a direct callback and when to use `do Event`

Prefer a direct callback when:

- the logic is only a small local state write,
- you need the callback payload immediately,
- there is no shared pending/success/error state to expose.

Prefer `do Event { ... }` when:

- several widgets should trigger the same action,
- the UI should bind to `running`, `result`, or `error`,
- you want a reusable effect handle instead of re-writing the same callback body.

A useful pattern is: callback functions gather payloads, signals hold live form state, and an event handle owns the actual submission effect.

## Chapter 7: Structural UI with `<show>` and `<each>`

Dynamic child structure uses mounted structural bindings rather than plain rerendering.

```aivi
use aivi.reactive
use aivi.ui.gtk4

rows = signal [
  { id: "1", title: "Inbox", visible: True },
  { id: "2", title: "Archive", visible: False }
]
sidebarOpen = signal True
visibleRows = rows ->> filter .visible

mailboxRow = row => ~<gtk>
  <GtkLabel label={row.title} />
</gtk>

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="6">
    <show when={sidebarOpen}>
      <GtkLabel label="Sidebar" />
    </show>
    <each items={visibleRows} as={row} key={row => row.id}>
      <MailboxRow row />
    </each>
  </GtkBox>
</gtk>
```

The public contract is:

- `<show>` mounts or disposes one child scope as its guard changes,
- `<each>` preserves one mounted child scope per key,
- keyed children move instead of being recreated when possible,
- inserts and removals go through the owning GTK container.

## Chapter 8: App lifecycle, windows, and lower-level helpers

### 8.1 Root windows and app startup

Any GTK or libadwaita class that is a `GtkWindow` subclass is a valid primary root node. In practice that usually means `GtkWindow`, `GtkApplicationWindow`, `AdwWindow`, `AdwApplicationWindow`, and concrete dialog/window subclasses.

Use `runGtkApp` for the common single-root case. Use `mountAppWindow` when you need the mounted `WindowId` directly or when you want multiple live roots under one app.

```aivi
use aivi.reactive
use aivi.ui.gtk4

settingsOpen = signal False

windowRoot = ~<gtk>
  <AdwApplicationWindow title="Mailfox">
    <GtkBox />
  </AdwApplicationWindow>
</gtk>

settingsDialog = ~<gtk>
  <AdwPreferencesDialog id="settingsDialog" open={settingsOpen}>
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

`mountAppWindow : AppId -> List GtkNode -> Effect GtkError WindowId` follows these rules:

- the list must contain at least one root,
- the first root is the primary app window and becomes the returned `WindowId`,
- later roots stay live under the same app/runtime,
- extra dialog roots may default to the primary window when the surrounding surface supports it.

### 8.2 Lower-level helpers

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
| `menuButtonSetMenuModel` | you are wiring programmatic GMenu infrastructure |

Bound callbacks and event handles are the default. These helpers are mainly for libraries, tests, embedding, or special GTK integrations.

### 8.3 Raw GTK events

`signalStream` and `signalPoll` use the public `GtkSignalEvent` surface:

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
  | GtkWindowClosed  WidgetId Text
  | GtkUnknownSignal WidgetId Text Text Text Text
  | GtkTick
```

The second field is the widget name from `id="..."`, or `""` when the widget has no name. That is why lower-level integrations usually match by name instead of numeric id.

A manual raw-event loop looks like this:

```aivi
use aivi.concurrency
use aivi.ui.gtk4

main = do Effect {
  rx <- signalStream Unit
  concurrency.forEach rx (event =>
    event match
      | GtkUnknownSignal _ _ "action" actionName _ => handleAction actionName
      | GtkTick                                     => pure Unit
      | _                                           => pure Unit
  )
}
```

## Chapter 9: Debugging a native app with the MCP server

The MCP server is the best way to inspect a running AIVI GTK app from an editor agent or other local automation client.

Start an MCP host against:

```text
aivi mcp serve . --ui --allow-effects
```

See [MCP Server](../../tools/mcp.md) for the full protocol and tool list. In practice, the native-app debugging loop looks like this.

### 9.1 Launch or attach

Launch a target app under inspection:

```json
{ "target": "demos/snake.aivi" }
```

Use that payload with `aivi_gtk_launch`. Save the returned `sessionId`.

If the app is already running, use `aivi_gtk_discover` to find candidate sockets, then `aivi_gtk_attach` once you have the matching `socketPath` and token.

### 9.2 Confirm the session and list widgets

Check that the session is alive:

```json
{ "sessionId": "<sessionId>" }
```

Use that payload with `aivi_gtk_hello`, then list stable widget handles with `aivi_gtk_listWidgets`.

This is where widget `id="..."` names pay off. Give important inputs, buttons, panes, and dialogs explicit names so you can target them by `name` instead of guessing numeric ids.

### 9.3 Inspect the UI tree and reactive state

For one widget, call `aivi_gtk_inspectWidget`:

```json
{ "sessionId": "<sessionId>", "name": "saveButton" }
```

For the whole mounted tree, call `aivi_gtk_dumpTree`.

When the problem is state rather than layout, inspect the reactive layer with `aivi_gtk_listSignals` and `aivi_gtk_inspectSignal`. This is especially useful for checking whether:

- an `onClick` or `onInput` handler wrote the expected signal,
- an `EventHandle` moved through `running`, `result`, or `error`,
- a derived signal is stale because the wrong source signal is feeding it.

### 9.4 Reproduce user input from the debugger

Use the mutation tools to drive the app the same way a user would:

- `aivi_gtk_click`
- `aivi_gtk_type`
- `aivi_gtk_focus`
- `aivi_gtk_moveFocus`
- `aivi_gtk_select`
- `aivi_gtk_scroll`
- `aivi_gtk_keyPress`

A typical flow is:

1. focus a known widget,
2. type or click,
3. re-read the widget with `inspectWidget`,
4. re-read the relevant signal with `inspectSignal`.

That lets you answer both halves of a UI bug: "did the host widget change?" and "did the reactive state graph change?"

### 9.5 A practical debugging checklist

When you build a GTK app that you expect to debug later, these habits help immediately:

- add `id="..."` to important widgets,
- keep event-handle names descriptive (`saveDraft`, `refreshMailbox`, `closeSettings`),
- derive display-only values with `->>` so they appear as first-class signals in inspection output,
- keep low-level raw-signal usage localized so the inspectable graph stays easy to read.

## Chapter 10: Coverage, boundaries, and diagnostics

### 10.1 What the declarative surface covers

The declarative GTK surface supports the full signal-first story plus a broad GTK4/libadwaita widget set.

The public contract is:

- any indexed `Gtk*` or `Adw*` class that is valid in the surrounding GTK API may be instantiated from `~<gtk>`, `buildFromNode`, or `buildWithIds`,
- writable scalar properties use existing tuned setters where needed and otherwise fall back to metadata-driven property application,
- object-valued properties can be expressed with nested `<property name="..."> <Gtk.../> </property>` nodes,
- common single-child containers can attach through common pointer properties such as `child` and `content`,
- container-specific `<child type="...">` attachment remains a thin handwritten layer for surfaces that need specialized add/remove calls,
- typed callback payloads stay curated for the common sugar surface, while broader indexed signals may still surface through `GtkUnknownSignal`.

That means the declarative tree is not limited to a tiny hand-wrapped widget set. In addition to `GtkBox`, `GtkButton`, and `GtkEntry`, the current surface also covers broader families such as `GtkListView`, `GtkColumnView`, `GtkGridView`, `GtkPopover`, `GtkPopoverMenu`, `GtkPopoverMenuBar`, `GtkVideo`, `GtkMediaControls`, `GtkStringList`, `GtkAdjustment`, `GtkSelectionModel` implementations, `GtkFilterListModel`, `GtkSortListModel`, `GtkSignalListItemFactory`, `GtkBuilderListItemFactory`, and layout-manager objects when they are connected through the right property.

### 10.2 Surfaces that stay programmatic

Some GTK and libadwaita APIs still make more sense as programmatic helpers than as ordinary tree nodes:

- `GtkAlertDialog` and `GtkFileDialog`, which are asynchronous helper objects,
- `GSimpleAction` and `GMenu`, which are application-level action/menu infrastructure,
- animation objects such as `AdwTimedAnimation` and `AdwSpringAnimation`,
- printing, settings, builder, and other lifecycle-oriented infrastructure such as `GtkPrintOperation`, `GtkSettings`, `GtkStyleContext`, `GtkBuilder`, and `GtkApplication`.

For action-driven integrations, raw runtime events can still surface through `signalStream`:

```aivi
handleRuntimeEvent = event => event match
  | GtkUnknownSignal _ _ "action" actionName _ => handleAction actionName
  | _                                           => pure Unit
```

### 10.3 Diagnostics

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

- [Signals](./reactive_signals.md) for the day-to-day reactive API
- [Reactive Dataflow](./reactive_dataflow.md) for batching, invalidation, and lifecycle cleanup
- [Forms](./forms.md) for typed field state and validation patterns on top of GTK callbacks
- [MCP Server](../../tools/mcp.md) for the full tool reference behind the debugging workflow above
