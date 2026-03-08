# `aivi.ui.forms`
## Lightweight Forms and Validation for GTK Apps

<!-- quick-info: {"kind":"module","name":"aivi.ui.forms"} -->
`aivi.ui.forms` gives GTK apps a small set of practical form helpers: typed field state with `Field A`, update helpers for common GTK input events, and validation helpers built on `Validation (List E) A`.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.forms</div>

This module is designed to fit naturally into [`gtkApp`](./app_architecture.md):

- the **model** owns the editable values,
- **`Msg`** constructors describe input, blur, and submit events,
- **`update`** changes field state with pure helpers,
- **`view`** renders inline feedback,
- **commands** can consume the validated result when submission needs IO.

If you know ordinary web-form patterns, think of `Field A` as “the input value plus the small amount of UI metadata you usually track by hand”.

## Public API

<<< ../../snippets/from_md/stdlib/ui/forms/block_01.aivi{aivi}


## `Field A`

`Field A` is the recommended model shape for one editable input.

- `value` is the current draft value,
- `touched` becomes `True` after the user leaves the field,
- `dirty` becomes `True` after any input change.

The module deliberately stays small. Instead of forcing a single giant `Form` type, it lets you keep your overall screen model as a normal AIVI record and use `Field A` only where it helps.

## The usual GTK event flow

A typical field follows this mapping:

| GTK event | `Msg` | Model helper |
| --- | --- | --- |
| `GtkInputChanged _ "fieldId" txt` | `FieldChanged txt` | `setValue txt field` |
| `GtkFocusOut _ "fieldId"` | `FieldBlurred` | `touch field` |
| `GtkClicked _ "submitBtn"` | `Submit` | `submitted: True` |

That keeps forms inside the same event pipeline as the rest of the app. There is no hidden widget-owned form state and no second validation loop to learn.

## Field-level validation

Validators stay as plain pure functions returning `Validation (List E) A`:

<<< ../../snippets/from_md/stdlib/ui/forms/block_02.aivi{aivi}


`minLength`, `maxLength`, and `email` treat empty input as valid so that `required` can be layered on separately. That avoids duplicate messages such as “required” and “too short” for the same blank field.

`visibleErrors submitted validator field` is the helper most apps want for rendering:

- before blur and before submit → `[]`,
- after blur → the field's current validation errors,
- after submit → all field errors, even for untouched fields.

## Form-level validation

Field-level checks are great for inline feedback. On submit, you usually want one typed domain value.

<<< ../../snippets/from_md/stdlib/ui/forms/block_03.aivi{aivi}


A good rule of thumb is:

1. keep editable draft state in the GTK app model,
2. show field errors with `visibleErrors`,
3. build the final domain value with `Validation`.

## Full GTK app example

<<< ../../snippets/from_md/stdlib/ui/forms/block_04.aivi{aivi}


This example stops before command execution on purpose. If submission should trigger IO, first produce the validated payload, then launch the command from `update`.
