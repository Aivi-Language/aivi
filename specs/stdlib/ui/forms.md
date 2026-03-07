# `aivi.ui.forms`
## Lightweight Forms and Validation for GTK Apps

> **Status: Phase 2 lightweight foundation**  
> `aivi.ui.forms` adds a first-class form story for the blessed GTK architecture without introducing a second UI runtime, schema DSL, or command system.

<!-- quick-info: {"kind":"module","name":"aivi.ui.forms"} -->
`aivi.ui.forms` provides lightweight building blocks for form-heavy GTK apps: typed field state with `Field A`, predictable `GtkInputChanged` / `GtkFocusOut` update helpers, and validation helpers built on top of `Validation (List E) A`.
<!-- /quick-info -->

<div class="import-badge">use aivi.ui.forms</div>

`aivi.ui.forms` is designed to sit directly on top of the blessed [`gtkApp`](./app_architecture.md) architecture:

- the **model** owns form state,
- **`Msg`** constructors represent input, blur, and submit events,
- **`update`** mutates field state with pure helpers,
- **`view`** renders inline errors from pure validation results,
- form submission still uses ordinary `Validation` composition and later command milestones can consume the typed validated result.

It intentionally stays small so that future schema-first data work can reuse these helpers instead of competing with them.

## Public API

```aivi
Field A = {
  value: A
  touched: Bool
  dirty: Bool
}

field : A -> Field A
setValue : A -> Field A -> Field A
touch : Field A -> Field A

validate : (A -> Validation (List E) B) -> Field A -> Validation (List E) B
errors : (A -> Validation (List E) B) -> Field A -> List E
visibleErrors : Bool -> (A -> Validation (List E) B) -> Field A -> List E

allOf : List (A -> Validation (List E) B) -> A -> Validation (List E) A
rule : Text -> (A -> Bool) -> A -> Validation (List Text) A
required : Text -> Validation (List Text) Text
minLength : Int -> Text -> Validation (List Text) Text
maxLength : Int -> Text -> Validation (List Text) Text
email : Text -> Validation (List Text) Text
```

### `Field A`

`Field A` is the recommended per-input model shape.

- `value` is the current editable value,
- `touched` becomes `True` after blur,
- `dirty` becomes `True` after any input change.

`aivi.ui.forms` does **not** prescribe one monolithic `Form` record type. Keep your overall screen model as an ordinary AIVI record and store `Field A` values only where you need editable state.

## GTK event flow

The expected mapping from GTK signals into form state is:

| GTK event | `Msg` | Model helper |
| --- | --- | --- |
| `GtkInputChanged _ "fieldId" txt` | `FieldChanged txt` | `setValue txt field` |
| `GtkFocusOut _ "fieldId"` | `FieldBlurred` | `touch field` |
| `GtkClicked _ "submitBtn"` | `Submit` | `submitted: True` |

That flow keeps forms inside the same event pipeline as every other GTK screen. There is no second subscription loop and no hidden mutable widget state.

## Field-level validation

Validators stay as ordinary pure functions returning `Validation (List E) A`.

```aivi
use aivi.validation
use aivi.ui.forms

displayName : Text -> Validation (List Text) Text
displayName = allOf [required, minLength 2]

bio : Text -> Validation (List Text) Text
bio = maxLength 120
```

`minLength`, `maxLength`, and `email` treat empty input as valid so that `required` can be layered on separately without duplicating "required" and "too short"/"invalid email" messages for blank fields.

`visibleErrors submitted validator field` is the recommended rendering helper:

- before blur and before submit → `[]`
- after blur → the field's current validation errors
- after submit → all field errors, even for untouched fields

## Form-level validation

Field-level checks control inline feedback. Form submission should still produce a typed domain value with the existing `Validation` applicative.

```aivi
Contact = {
  name: Text
  email: Text
}

MkContact : Text -> Text -> Contact
MkContact = name email => {
  name: name
  email: email
}

toContact : {
  name: Field Text
  email: Field Text
} -> Validation (List Text) Contact
toContact = model => ap (ap (Valid MkContact) (validate (allOf [required, minLength 2]) model.name)) (validate (allOf [required, email]) model.email)
```

This is the intended layering:

1. keep editable draft state in the GTK app model,
2. render field errors from `visibleErrors`,
3. produce the typed submit payload with `Validation`.

## Full blessed-architecture example

```aivi
use aivi
use aivi.text
use aivi.validation
use aivi.ui.forms
use aivi.ui.gtk4

Model = {
  submitted: Bool
  name: Field Text
  email: Field Text
}

Msg =
  | NameChanged Text
  | NameBlurred
  | EmailChanged Text
  | EmailBlurred
  | Submit

initialModel : Model
initialModel = {
  submitted: False
  name: field ""
  email: field ""
}

nameRule : Text -> Validation (List Text) Text
nameRule = allOf [required, minLength 2]

emailRule : Text -> Validation (List Text) Text
emailRule = allOf [required, email]

nameErrors : Model -> List Text
nameErrors = model => visibleErrors model.submitted nameRule model.name

emailErrors : Model -> List Text
emailErrors = model => visibleErrors model.submitted emailRule model.email

view : Model -> GtkNode
view = model => ~<gtk>
  <GtkBox orientation="vertical" spacing="8" marginTop="12" marginStart="12" marginEnd="12">
    <GtkEntry id="nameInput" text={model.name.value} placeholderText="Name" onInput={ NameChanged } onFocusOut={ NameBlurred } />
    <GtkLabel label={text.join ", " (nameErrors model)} />
    <GtkEntry id="emailInput" text={model.email.value} placeholderText="Email" onInput={ EmailChanged } onFocusOut={ EmailBlurred } />
    <GtkLabel label={text.join ", " (emailErrors model)} />
    <GtkButton id="submitBtn" label="Save" onClick={ Submit } />
  </GtkBox>
</gtk>

toMsg : GtkSignalEvent -> Option Msg
toMsg = event => event match
  | GtkInputChanged _ "nameInput" txt  => Some (NameChanged txt)
  | GtkFocusOut _ "nameInput"          => Some NameBlurred
  | GtkInputChanged _ "emailInput" txt => Some (EmailChanged txt)
  | GtkFocusOut _ "emailInput"         => Some EmailBlurred
  | GtkClicked _ "submitBtn"           => Some Submit
  | _                                  => None

update : Msg -> Model -> Effect GtkError Model
update = msg model => msg match
  | NameChanged txt =>
      pure (model <| { name: setValue txt model.name })
  | NameBlurred =>
      pure (model <| { name: touch model.name })
  | EmailChanged txt =>
      pure (model <| { email: setValue txt model.email })
  | EmailBlurred =>
      pure (model <| { email: touch model.email })
  | Submit =>
      pure (model <| { submitted: True })
```

This example intentionally stops before command execution. A later milestone may take the validated submit payload and run an effectful save command, but the form model itself already fits the blessed architecture today.
