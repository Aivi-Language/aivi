# `aivi.ui.forms`
## Lightweight Forms and Validation for GTK Apps

<!-- quick-info: {"kind":"module","name":"aivi.ui.forms"} -->
`aivi.ui.forms` gives GTK apps a small set of practical form helpers: typed field state with `Field A`, update helpers for common GTK input events, and validation helpers built on [`Validation`](../core/validation.md).
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.forms</div>

This module fits directly into signal-first GTK apps. Keep `Field A` values inside a signal or a record-valued signal, update them from widget callbacks, derive visible errors as signals, and use an `Event` handle when submission needs IO.

## Public API

<<< ../../snippets/from_md/stdlib/ui/forms/block_01.aivi{aivi}

## `Field A`

`Field A` is the recommended shape for one editable input.

- `value` is the current draft value,
- `touched` becomes `True` after the user leaves the field,
- `dirty` becomes `True` after any input change.

In practice, `touched` is usually what controls when validation feedback becomes visible, while `dirty` is useful for save-button state or unsaved-changes prompts.

## The usual GTK event flow

A typical field now stays inside direct signal updates:

| GTK binding | Typical signal update |
| --- | --- |
| `onInput={txt => ...}` | `update form (patch { name: setValue txt })` |
| `onFocusOut={_ => ...}` | `update form (patch { name: touch })` |
| `onClick={submitEvent}` | trigger an `Event` handle that reads the validated payload |

That keeps forms inside the same signal graph as the rest of the UI. There is no hidden widget-owned form state and no second validation loop to learn.

## Field-level validation

Validators stay plain pure functions. The built-in helpers mostly validate `Text`, but custom rules can use the same `Validation (List E) A` shape.

`visibleErrors submitted validator field` is still the helper most apps want for rendering:

- before blur and before submit -> `[]`,
- after blur -> the field's current validation errors,
- after submit -> all field errors, even for untouched fields.

In a signal-first app, the usual pattern is to derive the visible errors as another signal:

```aivi
nameErrors = form |> map (state =>
  visibleErrors state.submitted nameRule state.name
)
```

## Form-level validation

Field-level checks are great for inline feedback. On submit, you usually want one typed domain value, and `Validation` still lets you accumulate all field errors instead of stopping at the first one.

A good rule of thumb is:

1. keep editable draft state in a signal,
2. show field errors through derived signals,
3. build the final domain value with `Validation`,
4. run submission IO through an `Event` handle.

## Async submit pattern

When submission needs IO, keep the validated payload as plain data and let an `Event` handle own the effectful work:

```aivi
submitProfile : Event GtkError SavedProfile
submitProfile = event (saveProfile (buildProfile (get form)))
```

Because `submitProfile.running`, `submitProfile.result`, and `submitProfile.error` are signals, the UI can bind loading, success, and failure state directly.

## Where to go next

- [`aivi.ui.gtk4`](./gtk4.md) — how forms fit into mounted GTK bindings and signal-first app structure
- [Signals](./reactive_signals.md) — writable state, derived state, and watchers
- [`aivi.ui.gtk4`](./gtk4.md) — callback binding rules for `onInput`, `onFocusOut`, and submit buttons
