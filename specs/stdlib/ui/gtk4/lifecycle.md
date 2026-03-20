# Lifecycle

Part of the [Writing Native Apps](../gtk4.md) guide.

## Root windows and app startup

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

main =
  Unit
     |> init
     |> _ => appNew "com.example.mailfox" #app
     |> _ => mountAppWindow app [windowRoot, settingsDialog] #win
     |> _ => windowPresent win
     |> _ => appRun app
```

`mountAppWindow : AppId -> List GtkNode -> Effect GtkError WindowId` follows these rules:

- the list must contain at least one root,
- the first root is the primary app window and becomes the returned `WindowId`,
- later roots stay live under the same app/runtime,
- extra dialog roots may default to the primary window when the surrounding surface supports it.

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

Bound callbacks and event handles are the default. These helpers are mainly for libraries, tests, embedding, or special GTK integrations.

The imperative surface is intentionally narrow. Dialog-construction helpers, tray/mail helpers, D-Bus startup, badge counters, and theme probes are not public GTK APIs; use mounted dialog roots plus ordinary callbacks instead.

## Curated imperative integrations

### Programmatic GMenu setup

Use `menuModelNew`, `menuModelAppendItem`, and `menuButtonSetMenuModel` when you need GTK's application-level `GMenu` infrastructure. Keep the button itself declarative and attach the model after mount:

```aivi
use aivi.ui.gtk4

windowRoot = ~<gtk>
  <AdwApplicationWindow title="Mailfox">
    <GtkMenuButton id="appMenu" label="App" />
  </AdwApplicationWindow>
</gtk>

main =
  Unit
     |> init
     |> _ => appNew "com.example.mailfox" #app
     |> _ => actionNew "preferences" #action
     |> _ => appAddAction app action
     |> _ => mountAppWindow app [windowRoot] #win
     |> _ => menuModelNew Unit #menu
     |> _ => menuModelAppendItem menu "Preferences" "app.preferences"
     |> _ => widgetById "appMenu" #button
     |> _ => menuButtonSetMenuModel button menu
     |> _ => windowPresent win
     |> _ => appRun app
```

### Desktop handoff

`osOpenUri` is the public escape hatch for opening links or files with the desktop shell. Because it is callback-friendly, it works directly from the signal-first app shape:

```aivi
docsButton = ~<gtk>
  <GtkButton
    label="Open docs"
    onClick={_ => osOpenUri "https://example.com/docs"}
  />
</gtk>
```

### Low-level tick feeds

Use `gtkSetInterval` only when direct callbacks or event handles are not a good fit and you explicitly want raw `GtkTick` events:

```aivi
use aivi.concurrency

main =
  Unit
     |> _ => gtkSetInterval 1000
     |> _ => signalStream Unit #rx
     |> _ => concurrency.forEach rx (event =>
          event match
            | GtkTick => pollExternalState
            | _       => pure Unit
        )
```

## Raw GTK events

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

```aivi
use aivi.concurrency
use aivi.ui.gtk4

main =
  Unit
     |> _ => signalStream Unit #rx
     |> _ => concurrency.forEach rx (event =>
          event match
            | GtkUnknownSignal _ _ "action" actionName _ => handleAction actionName
            | GtkTick                                     => pure Unit
            | _                                           => pure Unit
        )
```

Next: [MCP Debugging](./mcp_debugging.md)
