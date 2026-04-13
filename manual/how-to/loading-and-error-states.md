# How to model loading and error states

When a screen can be loading, ready, or failed, model that shape directly in the type instead of
smuggling the meaning through booleans and empty lists.

## Example

```aivi
type Screen A =
  | Loading
  | Ready A
  | Failed Text

type List Text -> Text
func readyHeadline = tasks => tasks
  |> length
  |> "{.} tasks ready"

type Screen (List Text) -> Text
func headline = screen => screen
 ||> Loading      -> "Loading tasks..."
 ||> Ready tasks  -> readyHeadline tasks
 ||> Failed error -> "Could not load tasks: {error}"

value screen : Screen (List Text) =
    Ready [
        "Write the tutorial",
        "Ship the docs"
    ]

value main =
    <Window title="Tasks">
        <Label text={headline screen} />
    </Window>

export main
```

## Why this is better than flags

- The type tells readers exactly which states exist.
- Pattern matching forces you to handle every state.
- You do not need to guess what `(isLoading, error, items)` means in a half-initialized moment.

## Practical advice

- Use a sum type when loading is part of the user experience, not just an implementation detail.
- Keep the error payload concrete enough to render useful UI.
- Derive display signals from the screen state instead of scattering special cases through the UI.
