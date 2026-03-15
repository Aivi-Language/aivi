# Mental Model

Part of the [Writing Native Apps](../gtk4.md) guide.

AIVI native apps are built from five pieces that fit together directly:

- `Signal` values hold live app state.
- `signal ->> ...` derives live read-only state.
- `signal <<- ...` writes live state.
- `~<gtk>...</gtk>` describes the mounted GTK tree.
- callbacks and `do Event { ... }` values connect user input to state changes and effects.

GTK runtime APIs are ordinary AIVI values and effects. A good default is:

1. keep authoritative state in signals,
2. derive display-only data with `->>`,
3. render with the GTK sigil,
4. update signals from callbacks,
5. use `do Event { ... }` when a callback should own shared effect state.

## Choose your starting point

| If you need to... | Start with... |
| --- | --- |
| build a normal single-window app | `runGtkApp` plus a root `~<gtk>` tree |
| keep UI state live | `signal`, `->>`, `<<-` |
| wire user input from common widgets | callback sugar such as `onClick` and `onInput` |
| run one shared effectful action from several widgets | `do Event { ... }` and pass the handle to the widgets |
| show or repeat dynamic child content | `<show>` and `<each key={...}>` |
| debug or test below the sugar layer | `buildWithIds`, `signalStream`, `signalPoll`, `signalEmit` |

Next: [First Window](./first_window.md)
