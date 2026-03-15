# State & Patches

Part of the [Writing Native Apps](../gtk4.md) guide.

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

## Operator rules

- `value <| { ... }` updates ordinary immutable data and returns a new value.
- `signal <<- value` replaces the signal's value.
- `signal <<- fn` updates from the previous value.
- `signal <<- { ... }` applies patch semantics to the current record value.
- `signal ->> rhs` is shorthand for deriving a signal whose mapper reads like `value |> rhs`.

## More than one signal

When a derived value depends on several signals, use `combineAll`:

```aivi
title = signal ""
saveBusy = signal False

canSave = combineAll (title, saveBusy) ((currentTitle, busy) =>
  currentTitle != "" && !busy
)
```

Use `->>` for one signal, `combineAll` for several.

Next: [GTK Sugar](./gtk_sugar.md)
