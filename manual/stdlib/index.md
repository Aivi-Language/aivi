# Standard Library

The AIVI standard library catalog is being built out alongside the language implementation.
This page will be the full reference once modules stabilize.

## Planned top-level modules

| Module | Description |
|---|---|
| `aivi.core` | Fundamental types: `Maybe`, `Result`, `Ordering`, `Unit` |
| `aivi.text` | Text operations: `length`, `contains`, `split`, `trim`, `toUpper`, `toLower`, `toInt`, `fromInt` |
| `aivi.list` | List operations: `map`, `filter`, `fold`, `any`, `all`, `head`, `tail`, `length`, `append`, `zip`, `partition` |
| `aivi.math` | Arithmetic helpers: `abs`, `max`, `min`, `clamp`, `round`, `floor`, `ceil` |
| `aivi.network` | HTTP sources: `http.get`, `http.post`, `http.put`, `http.delete`, `socket` |
| `aivi.fs` | File system sources: `file.read`, `file.watch`, `dir.list` |
| `aivi.timer` | Timer sources: `timer.every`, `timer.after`, `timer.debounce` |
| `aivi.window` | Window events: `window.keyDown`, `window.keyUp`, `window.resize`, `window.focus` |
| `aivi.clipboard` | Clipboard: `clipboard.read`, `clipboard.write` |
| `aivi.dialog` | GTK dialogs: `dialog.open`, `dialog.save`, `dialog.alert`, `dialog.confirm` |
| `aivi.gtk` | Low-level GTK widget bindings for advanced use cases |

## Importing modules

```aivi
use aivi.network (
    http
)

use aivi.list (
    filter
    map
    fold
)
```

`use` brings specific names into scope. You can also use the module path directly:

```aivi
val filtered = List.filter (\x => x > 0) myList
val joined   = Text.join ", " labels
```

## Current status

The core language and basic types (`Maybe`, `Result`, `List`, `Bool`, `Int`, `Text`) are
implemented. Network, filesystem, and GTK-specific modules are under active development.

Check the [GitHub repository](https://github.com/mendrik/aivi2) for the latest status.
