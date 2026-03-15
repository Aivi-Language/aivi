# Callbacks

Part of the [Writing Native Apps](../gtk4.md) guide.

Common GTK signals have direct sugar attributes. Use them when you want readable app code and typed payloads.

## Which handler shape to use

Every sugared callback position accepts one of two authoring styles:

1. a function callback, when you need the payload,
2. an `EventHandle`, when the signal itself should trigger the handle's `.run` effect.

That leads to a simple rule:

- if you need the current text, bool, index, float, or key event, write a function,
- if you just want "run this action now", pass the event handle directly.

## Sugar mapping and payloads

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

## Direct payload callbacks

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

## Keyboard callbacks

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

## Raw signal escape hatch

When there is no sugared attribute, or when you want the raw `GtkSignalEvent`, use explicit `<signal ... />` nodes.

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

Next: [Events](./events.md)
