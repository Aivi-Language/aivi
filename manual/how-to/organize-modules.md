# How to organize modules

Split code into modules when a concept deserves a stable name, a focused file, or a reusable export.

## A small example

`tasks.aivi`

```text
type Todo = {
    text: Text,
    done: Bool
}

type Todo -> Bool
func isOpen = todo =>
    todo.done == False

export (Todo, isOpen)
```

`main.aivi`

```text
use tasks (
    Todo
    isOpen
)

value items : List Todo = [
    {
        text: "Write docs",
        done: False
    },
    { text: "Ship app", done: True }
]

value openCount = items
  |> filter isOpen
  |> length

export openCount
```

## Good module boundaries

- Put **shared types** in their own module when several files talk about the same data.
- Put **domain logic** next to the type it belongs to.
- Keep **UI assembly** close to the screen that uses it.
- Export the small surface other files should depend on, not every helper you wrote.

For the full language surface around imports and exports, see [Modules](/guide/modules).
