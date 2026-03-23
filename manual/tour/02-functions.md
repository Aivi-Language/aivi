# Functions

Functions in AIVI are declared with `fun`. They are pure by default — a function depends only
on its explicit parameters and always returns the same result for the same inputs.

## Basic syntax

```aivi
fun add:Int #x:Int #y:Int =>
    x + y
```

Breaking this down:

| Part | Meaning |
|---|---|
| `fun` | declaration keyword |
| `add` | function name |
| `:Int` | **return type** (comes before parameters) |
| `#x:Int` | labeled parameter named `x` of type `Int` |
| `#y:Int` | labeled parameter named `y` of type `Int` |
| `=>` | separates signature from body |
| `x + y` | function body — any expression |

The return type prefix (`add:Int`) is one of AIVI's deliberate design choices:
reading left to right, you see the name and return type before the parameters.

## Labeled parameters

All parameters in AIVI are labeled with `#`. This means you always know what an argument
represents at the call site:

```aivi
fun greet:Text #name:Text #title:Text =>
    "Hello, {title} {name}!"

val msg = greet "Lovelace" "Ms"    -- ERROR: unlabeled
val msg = greet #name="Lovelace" #title="Ms"   -- correct
```

Wait — that does not look like the snake demo. Let me clarify: labeled parameters can be
applied **positionally** or by name, as long as every parameter is supplied.
In practice, most AIVI code passes arguments positionally because the parameter names are
visible in the function declaration:

```aivi
-- these two calls are equivalent:
val result = greet "Lovelace" "Ms"
val result = greet #name="Lovelace" #title="Ms"
```

The `#` sigil on the parameter declaration signals that this is a labeled parameter.
This is visible in syntax highlighting — labeled params appear in a distinct colour.

## Multi-parameter functions

```aivi
fun clamp:Int #lo:Int #hi:Int #value:Int =>
    value
     ||> v => lo
          when v < lo
     ||> v => hi
          when v > hi
     ||> v => v
```

Functions can have as many labeled parameters as needed.

## Calling functions

Pass arguments positionally after the function name:

```aivi
val clamped = clamp 0 100 42       -- result: 42
val low     = clamp 0 100 (-5)     -- result: 0
```

When the argument is a complex expression, wrap it in parentheses:

```aivi
val moved = nextHead direction snake.head
val eaten = willEat boardSize direction food snake
```

Note that in AIVI, function application is juxtaposition (no parentheses for the call itself,
only for grouping subexpressions). `nextHead direction snake.head` calls `nextHead` with two
arguments: `direction` and `snake.head`.

## Functions as values

Functions in AIVI are first-class. You can pass a function as an argument:

```aivi
fun applyTwice:Int #f:(Int -> Int) #x:Int =>
    f (f x)

val result = applyTwice (\n => n + 1) 5   -- result: 7
```

The `->` in `(Int -> Int)` is a function type: a function that takes `Int` and returns `Int`.

## Anonymous functions (lambdas)

```aivi
val double = \x => x * 2
val add5   = \n => n + 5
```

Lambdas use the `\` syntax. They appear frequently in pipe chains:

```aivi
sig labelText : Signal Text =
    count
     |> \n => "You clicked {n} times"
```

## Pure by default

Every `fun` is pure. It cannot read from disk, make network calls, or mutate state.
Side effects live in `sig` declarations with `@source` decorators.

This means AIVI functions are easy to reason about and easy to test:
- given the same inputs, they always return the same output
- they have no hidden dependencies
- they can be called in any order without affecting each other

```aivi
fun double:Int #x:Int => x * 2
fun square:Int #x:Int => x * x

val test1 = double 5    -- always 10
val test2 = square 4    -- always 16
```

## Summary

- `fun name:ReturnType #param:Type => body`
- Return type comes immediately after the name, before parameters.
- All parameters are labeled with `#`.
- Function application is juxtaposition; parentheses group subexpressions.
- Lambdas: `\param => body`.
- Functions are pure — no side effects.

[Next: Pipes →](/tour/03-pipes)
