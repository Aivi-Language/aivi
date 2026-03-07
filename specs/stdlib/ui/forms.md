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

```aivi
use aivi.validation
use aivi.ui.forms

displayName : Text -> Validation (List Text) Text
displayName =
  // Require a value and make sure it is long enough to feel intentional.
  allOf [required, minLength 2]

bio : Text -> Validation (List Text) Text
bio =
  // Empty is fine here, but very long bios are not.
  maxLength 120
```

`minLength`, `maxLength`, and `email` treat empty input as valid so that `required` can be layered on separately. That avoids duplicate messages such as “required” and “too short” for the same blank field.

`visibleErrors submitted validator field` is the helper most apps want for rendering:

- before blur and before submit → `[]`,
- after blur → the field's current validation errors,
- after submit → all field errors, even for untouched fields.

## Form-level validation

Field-level checks are great for inline feedback. On submit, you usually want one typed domain value.

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
toContact = model =>
  // Validate each field, then assemble a typed Contact value.
  ap
    (ap (Valid MkContact) (validate (allOf [required, minLength 2]) model.name))
    (validate (allOf [required, email]) model.email)
```

A good rule of thumb is:

1. keep editable draft state in the GTK app model,
2. show field errors with `visibleErrors`,
3. build the final domain value with `Validation`.

## Full GTK app example

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
    <GtkEntry
      id="nameInput"
      text={model.name.value}
      placeholderText="Name"
      onInput={ NameChanged }
      onFocusOut={ NameBlurred }
    />
    <GtkLabel label={text.join ", " (nameErrors model)} />
    <GtkEntry
      id="emailInput"
      text={model.email.value}
      placeholderText="Email"
      onInput={ EmailChanged }
      onFocusOut={ EmailBlurred }
    />
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
  | _                                   => None

update : Msg -> Model -> Effect GtkError Model
update = msg model => msg match
  | NameChanged txt =>
      // Keep the latest draft value in the model.
      pure (model <| { name: setValue txt model.name })
  | NameBlurred =>
      // Mark the field as touched so its errors can become visible.
      pure (model <| { name: touch model.name })
  | EmailChanged txt =>
      pure (model <| { email: setValue txt model.email })
  | EmailBlurred =>
      pure (model <| { email: touch model.email })
  | Submit =>
      // Flip the submitted flag so every field shows its current errors.
      pure (model <| { submitted: True })
```

This example stops before command execution on purpose. If submission should trigger IO, first produce the validated payload, then launch the command from `update`.
