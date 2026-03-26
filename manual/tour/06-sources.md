# Sources

Signals describe *how* a value is computed. Sources describe *where* values come from.

A source is an external event stream — a keyboard, a timer, a network response, a file watcher.
Sources are attached to signals using the `@source` decorator.

## The @source decorator

```aivi
@source window.keyDown with {
    repeat: False,
    focusOnly: True
}
sig keyDown : Signal Key
```

`@source` names the provider (`window.keyDown`) and passes a configuration record in
`with { ... }`. The decorator binds to the **next** `sig` declaration below it.

## Source lifecycle

AIVI manages source lifecycle automatically:

1. When the component that owns the signal is mounted, the source is activated.
2. Each event from the source publishes a new payload into the input signal.
3. Downstream `scan` or derived signals react to those updates.
4. When the component is unmounted, the source is torn down.

You never subscribe or unsubscribe manually.

## Timer sources

A timer source is just a bodyless input signal that publishes `Unit` at a fixed interval:

```aivi
fun stepGame:Game tick:Unit game:Game =>
    game

@source timer.every 160ms with {
    immediate: True
}
sig tick : Signal Unit

sig game : Signal Game =
    tick
     |> scan initialGame stepGame
```

The interval uses the `Duration` domain literal (`ms`, `sec`, `min`). `tick` is the raw timer
stream; `scan` turns those timer events into accumulated state.

Options for `@source timer.every N`:

| Option | Type | Meaning |
|---|---|---|
| `immediate` | `Bool` | Fire once on activation before the first tick |
| `coalesce` | `Bool` | Drop accumulated ticks when the handler is busy |

## `@recur.backoff` — explicit retry recurrence

`@recur.backoff` drives a `Task E A` recurrence that retries on failure with exponential
back-off:

```aivi
@recur.backoff 3x
val fetched : Task HttpError User =
    initialState
     @|> fetchUser
     <|@ fetchUser
```

The retry count uses the `Retry` domain literal (`x`). This is explicit recurrence, not the
ordinary source-to-state pattern used by `scan`.

## `window.keyDown` — keyboard events

```aivi
@source window.keyDown with {
    repeat: False,
    focusOnly: True
}
sig keyDown : Signal Key
```

Emits a `Key` value on every key press. `repeat: False` suppresses held-key repeats.
`focusOnly: True` only fires when the window has focus.

## `http.get` / `http.post` — HTTP requests

```aivi
@source http.get "{apiHost}/users" with {
    headers: authHeaders,
    decode: Strict,
    retry: 3x,
    timeout: 5s
}
sig users : Signal (Result HttpError (List User))
```

The signal type is `Signal (Result HttpError A)`. It holds the latest response, or the
latest error if the request failed. `decode` controls JSON decoding strictness (`Strict`
or `Permissive`). `retry` and `timeout` use domain literals from `aivi.http`.

## `fs.watch` — filesystem events

```aivi
@source fs.watch "/tmp/demo.txt" with {
    events: [Created, Changed, Deleted]
}
sig fileEvents : Signal FsEvent
```

Emits `FsEvent` values (`Created`, `Changed`, `Deleted`) as the watched path changes.
Import `FsEvent` and its constructors from `aivi.fs`.

## `process.spawn` — subprocess output

```aivi
@source process.spawn "rg" ["TODO", "."] with {
    stdout: Lines,
    stderr: Ignore
}
sig grepEvents : Signal ProcessEvent
```

Spawns a child process and streams its output as `ProcessEvent` values. `stdout` and
`stderr` accept `Lines`, `Bytes`, or `Ignore`.

## Source configuration reference

| Source | Emits | Key options |
|---|---|---|
| `timer.every N` | `TimerTick` | `immediate`, `coalesce` |
| `window.keyDown` | `Key` | `repeat`, `focusOnly` |
| `http.get "url"` | `Result HttpError A` | `headers`, `decode`, `retry`, `timeout` |
| `http.post "url"` | `Result HttpError A` | `body`, `headers`, `decode`, `retry`, `timeout` |
| `fs.watch "path"` | `FsEvent` | `events` |
| `process.spawn "cmd" args` | `ProcessEvent` | `stdout`, `stderr` |

## How sources feed signals

1. `@source` declares the external event stream and decorates a bodyless `sig`.
2. `scan` folds those source events into state when the signal needs memory.
3. Pure derived signals (`|>` chains) recompute automatically when their dependency changes.
4. Markup binds to signals with `{signalName}` attributes.
5. GTK widgets re-render when bound signals change.

Everything between step 1 and step 5 is managed by the AIVI runtime.

## Stale suppression

If a source fires faster than the handler can process, `coalesce: True` drops intermediate
events and delivers only the latest. Essential for high-frequency timers.

## Summary

- `@source provider.name ...` decorates the next bodyless `sig` declaration.
- Use `upstream |> scan seed step` to accumulate source events into signal state.
- `@recur.backoff Nx` drives an explicit retrying recurrence for `Task`.
- Sources are activated on mount and torn down on unmount automatically.
- All source types emit typed values — no raw events reach user code.

[Next: Markup →](/tour/07-markup)
