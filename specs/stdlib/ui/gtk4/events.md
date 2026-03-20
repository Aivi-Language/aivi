# Events

Part of the [Writing Native Apps](../gtk4.md) guide.

`event.from` lifts a flow-shaped handler into an `EventHandle E A`. The resulting value carries lifecycle state as signals.

## The public shape

`EventHandle E A` has these fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `run` | `Effect E A` | Runs the underlying effect now. |
| `result` | `Signal (Option A)` | Last successful result, if any. |
| `error` | `Signal (Option E)` | Last failure, if any. |
| `done` | `Signal Bool` | `True` after the handle has completed. |
| `running` | `Signal Bool` | `True` while the handle is currently running. |

## A full event-handle example

```aivi
use aivi.reactive
use aivi.ui.gtk4

draft = signal { title: "Inbox" }

saveDraft : EventHandle GtkError Text
saveDraft =
  event.from (_ =>
    get draft
       |> persistDraft
       |> _ => "Saved"
  )

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

## When to use a callback and when to use an event handle

Prefer a direct callback when:

- the logic is only a small local state write,
- you need the callback payload immediately,
- there is no shared pending/success/error state to expose.

Prefer `event.from (...)` when:

- several widgets should trigger the same action,
- the UI should bind to `running`, `result`, or `error`,
- you want a reusable effect handle instead of re-writing the same callback body.

A useful pattern is: callback functions gather payloads, signals hold live form state, and an event handle owns the actual submission effect.
