# Forms

Forms are a classic source of complexity in UI code: each field has its own state, validation
must run at the right time, and the submit button should only be active when everything is valid.

In AIVI, each field is a signal, validation is a gate (`?\|>`), and the combined form state
is a derived signal.

## One signal per field

Declare a signal for each form field, driven by an `@source input.changed` event:

```aivi
@source input.changed "name-field"
sig rawName : Signal Text

@source input.changed "email-field"
sig rawEmail : Signal Text

@source input.changed "bio-field"
sig rawBio : Signal Text
```

Each source fires whenever the user types in the corresponding input widget.

## Validating with ?\|>

`?\|>` is the gate pipe: the value passes through only when the predicate is `True`.
A validated signal only has a value when the field is valid:

```aivi
fun isValidEmail:Bool #email:Text =>
    email
     |> Text.contains "@"

fun isNonEmpty:Bool #text:Text =>
    text != ""

sig validName : Signal Text =
    rawName
     ?|> isNonEmpty

sig validEmail : Signal Text =
    rawEmail
     ?|> isNonEmpty
     ?|> isValidEmail
```

`validName` only has a value when `rawName` is non-empty.
`validEmail` only has a value when `rawEmail` is non-empty AND contains `@`.

When a signal has no value (because a gate suppressed it), downstream signals depending on it
also have no value.

## Combining fields into a form signal

Once each field is validated, combine them into a single record signal:

```aivi
type ProfileForm = {
    name:  Text,
    email: Text,
    bio:   Text
}

sig validForm : Signal ProfileForm =
    validName
     &|> \name =>
        validEmail
         &|> \email =>
            validBio
             |> \bio => { name, email, bio }
```

`&\|>` is the apply pipe — it combines two signals. `validForm` only has a value when all
three fields are valid simultaneously.

## Enabling the submit button

```aivi
sig canSubmit : Signal Bool =
    validForm
     |> \_ => True
```

Or, if you want the button to show a disabled state rather than be absent:

```aivi
sig submitEnabled : Signal Bool =
    rawName
     |> isNonEmpty
     &|> \nameOk =>
        rawEmail
         |> \e => isNonEmpty e and isValidEmail e
         |> \emailOk => nameOk and emailOk
```

## Wiring submission

```aivi
@source button.clicked "submit"
sig submitClicked : Signal Unit

sig submittedForm : Signal ProfileForm =
    validForm
     ?|> \_ => True    -- gate: only pass if form is valid (it already is by construction)

-- In a real app, submittedForm would feed into an http.post source
```

## Full example

```aivi
type ContactForm = {
    name:    Text,
    message: Text
}

@source input.changed "name-input"
sig rawName : Signal Text

@source input.changed "message-input"
sig rawMessage : Signal Text

sig validName : Signal Text =
    rawName
     ?|> \t => t != ""

sig validMessage : Signal Text =
    rawMessage
     ?|> \t => Text.length t > 10

sig canSubmit : Signal Bool =
    rawName
     |> \n => n != ""
     &|> \nameOk =>
        rawMessage
         |> \m => Text.length m > 10
         |> \msgOk => nameOk and msgOk

@source button.clicked "submit"
sig submitClicked : Signal Unit

val main =
    <Window title="Contact">
        <Box orientation={Vertical} spacing={12}>
            <Entry id="name-input" placeholder="Your name" />
            <Entry id="message-input" placeholder="Your message" />
            <Button id="submit" label="Send" sensitive={canSubmit} />
        </Box>
    </Window>

export main
```

The `sensitive` attribute on `<Button>` controls whether it is clickable.
It is bound to `canSubmit`, so the button enables itself the moment both fields are valid.

## Summary

- One `sig` per field, driven by `@source input.changed`.
- `?\|>` gates filter to valid values only.
- Combine validated fields with `&\|>` into a form record signal.
- `canSubmit` is a derived boolean signal.
- Bind `sensitive={canSubmit}` to the submit button.
