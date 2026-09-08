# How to organize modules

Split code into modules when a concept deserves a stable name, a focused file, or a reusable export.

## A small example

`tasks.aivi`

```aivi-fragment
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

```aivi-fragment
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

Save these as two separate files in the same project directory, then run `aivi check main.aivi`.
They are highlighted as AIVI fragments because the single-block manual checker cannot resolve
the separate `tasks.aivi` module from the second block alone.

- Put **shared types** in their own module when several files talk about the same data.
- Put **domain logic** next to the type it belongs to.
- Keep **UI assembly** close to the screen that uses it.
- Export the small surface other files should depend on, not every helper you wrote.

For the full language surface around imports and exports, see [Modules](/guide/modules).
