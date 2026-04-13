# If you are new to functional programming

You do not need a functional-programming background to learn AIVI. You do need to accept that some
familiar tools are replaced by different ones:

| Familiar habit | In AIVI |
| --- | --- |
| Change a variable over time | Derive a new value, or move changing state into a `signal` |
| Write `if` / `else` chains | Pattern-match with `||>` or use `T|>` / `F|>` |
| Use loops for collections | Use `map`, `filter`, `reduce`, and friends |
| Hide effects in library calls | Declare sources and let the runtime bridge them into the graph |

## Values are simpler than variables

A variable makes you ask, *"what does this name mean right now?"* A value removes that question.

```aivi
value score = 10
value boostedScore = score + 5
```

`score` never becomes 15. `boostedScore` is a different value. That sounds small, but it removes an
entire category of mental bookkeeping.

## State still exists; it just moves

Functional programming does not ban change. It gives change a clearer home.

```aivi
signal count = 21

signal label = count
  |> "Count: {.}"
```

The value of `label` changes over time, but the relationship stays fixed and visible in the source.

## Branching becomes data-oriented

Instead of writing control flow around values, you branch on the value itself.

```aivi
type LoadState =
  | Loading
  | Ready Text
  | Failed Text

type LoadState -> Text
func describe = state => state
 ||> Loading      -> "Loading..."
 ||> Ready data   -> "Ready: {data}"
 ||> Failed error -> "Error: {error}"
```

The compiler checks that every case is handled. That is one of the main ergonomic wins of closed
types.

## Names should earn their keep

Beginners are often shown functional code full of clever temporary names. That is not the goal here.
In AIVI, use the simplest readable form first:

```aivi
type Text -> Text
func greeting = name => name
  |> trim
  |> "Hello, {.}!"
```

Reach for `#name` only when a later stage truly needs an earlier value or when two versions of the
same value would otherwise become confusing.

## What to read next

- [Start Here](/guide/getting-started) for the first-pass orientation
- [Build a Small Task Tracker](/guide/your-first-app) for the practical tutorial
- [Thinking in AIVI](/guide/thinking-in-aivi) for the deeper mental-model shift
