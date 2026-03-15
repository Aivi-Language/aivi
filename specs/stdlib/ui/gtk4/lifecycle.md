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
| `menuButtonSetMenuModel` | you are wiring programmatic GMenu infrastructure |

Bound callbacks and event handles are the default. These helpers are mainly for libraries, tests, embedding, or special GTK integrations.

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

Next: [MCP Debugging](./mcp_debugging.md)
