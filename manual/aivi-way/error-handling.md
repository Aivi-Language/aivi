# Error Handling

AIVI has no exceptions. Errors are values.

This is not a limitation — it is a design. When errors are values, the type system enforces
that you handle them. Nothing can go wrong silently.

## Result A: the error type

```aivi
type Result A = Ok A | Err Text
```

A `Result A` is either a successful value (`Ok A`) or an error message (`Err Text`).
Every operation that can fail returns `Result`.

## Matching on results

Use `\|\|>` to branch on `Ok` vs `Err`:

```aivi
fun describeResult:Text #result:Result Int =>
    result
     ||> Ok n    => "Success: {n}"
     ||> Err msg => "Failed: {msg}"
```

The compiler ensures you handle both cases. You cannot accidentally ignore an error.

## Chaining operations that might fail

A common pattern is a sequence of operations where each step can fail:

```aivi
fun parseAge:Result Int #input:Text =>
    input
     |> Text.toInt
     ?|> \n => n > 0 and n < 150

fun validateUser:Result User #raw:RawInput =>
    parseAge raw.ageText
     ||> Ok age  => Ok { name: raw.name, age }
     ||> Err msg => Err "Invalid age: {msg}"
```

## Propagating errors in signals

When a signal holds a `Result`, downstream signals can propagate the `Ok` value or branch
on the `Err`:

```aivi
@source http.get "/api/profile"
sig profileResult : Signal (Result Profile)

sig profileName : Signal Text =
    profileResult
     ||> Ok p    => p.name
     ||> Err _   => "Unknown"

sig profileError : Signal (Maybe Text) =
    profileResult
     ||> Ok _    => None
     ||> Err msg => Some msg
```

## Showing errors in markup

```aivi
sig hasError : Signal Bool =
    profileError
     ||> Some _ => True
     ||> None   => False

sig errorText : Signal Text =
    profileError
     ||> Some msg => msg
     ||> None     => ""

val profileView =
    <Box orientation={Vertical} spacing={8}>
        <show when={hasError}>
            <Label text={errorText} cssClass="error-label" />
        </show>
        <Label text={profileName} />
    </Box>
```

## The Maybe type for optional values

`Maybe A` handles absence (not failure):

```aivi
type Maybe A = Some A | None

sig selectedItem : Signal (Maybe Item)

sig selectionLabel : Signal Text =
    selectedItem
     ||> Some item => "Selected: {item.name}"
     ||> None      => "Nothing selected"
```

Use `Result` when an operation attempted and failed.
Use `Maybe` when a value is simply optional.

## Never throw

There is no `throw` in AIVI. Functions that encounter error conditions return `Err msg`.
Callers handle it explicitly.

This means:
- Reading a source file: returns `Result Text`.
- Parsing a number: returns `Result Int`.
- HTTP requests: return `Result Response`.
- Looking up a key in a map: returns `Maybe Value`.

The return type tells you whether the operation can fail before you even read the documentation.

## Recovering from errors

To fall back to a default value when a result is an error:

```aivi
fun withDefault:A #default:A #result:Result A =>
    result
     ||> Ok value => value
     ||> Err _    => default

val name = withDefault "Anonymous" profileResult
```

Or inline in a pipe:

```aivi
sig displayName : Signal Text =
    profileResult
     ||> Ok profile => profile.name
     ||> Err _      => "Anonymous"
```

## Collecting errors from a list

When validating a list of items, collect all errors rather than stopping at the first:

```aivi
fun validateAll:Result (List B) #validate:(A -> Result B) #items:List A =>
    items
     *|> validate
     |> List.partition
     ||> { errors: [], oks } => Ok oks
     ||> { errors }          => Err (errors |> List.head |> withDefault "Validation failed")
```

## Summary

- AIVI has no exceptions. Errors are `Result A = Ok A | Err Text`.
- Use `\|\|>` to branch on `Ok` vs `Err`. The compiler enforces exhaustiveness.
- `Maybe A = Some A | None` for optional values.
- Chain results with `\|\|>` arms that produce new `Result` values.
- `withDefault` recovers a fallback when a result is an error.
- Return type signatures communicate failure potential before reading docs.
