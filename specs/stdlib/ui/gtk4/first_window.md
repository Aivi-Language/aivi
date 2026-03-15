# First Window

Part of the [Writing Native Apps](../gtk4.md) guide.

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

## What to notice

- `state` is the source of truth.
- `title` is another signal, created with `->>`.
- the button callback writes the signal with `<<-`.
- the label binds directly to a signal.
- `id="incrementButton"` gives the widget a stable debug name for MCP and lower-level event matching.

Next: [State & Patches](./state_patches.md)
