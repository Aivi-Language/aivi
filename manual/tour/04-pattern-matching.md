# Pattern Matching

Pattern matching is AIVI's mechanism for inspecting the **shape** of a value and branching
based on what you find. It is more powerful than a `switch` statement because it matches
structure, not just equality.

## The match pipe \|\|>

`\|\|>` is the case pipe. It takes a value on the left and a pattern `=>` body arm on the right.
Multiple arms are written as successive `\|\|>` lines:

```aivi
fun directionText:Text #direction:Direction =>
    direction
     ||> Up    => "up"
     ||> Down  => "down"
     ||> Left  => "left"
     ||> Right => "right"
```

Each arm is: `\|\|> pattern => expression`.
The value (`direction`) is matched top-to-bottom against each pattern.
The body of the first matching arm is evaluated and returned.

## Matching on constructors

The most common use is matching on sum type variants:

```aivi
type Status = Running | Paused | GameOver

fun statusLabel:Text #status:Status =>
    status
     ||> Running  => "In progress"
     ||> Paused   => "Paused"
     ||> GameOver => "Game over"
```

## Exhaustiveness

Pattern matching in AIVI is **exhaustive**: the compiler rejects any match that does not
cover all variants. If you add a new variant to a sum type, every match on that type becomes
a compile error until you handle the new case.

This is the key advantage over `switch` statements: you cannot accidentally forget a case.

```aivi
type Color = Red | Green | Blue

fun colorName:Text #color:Color =>
    color
     ||> Red   => "red"
     ||> Green => "green"
     -- ERROR: Blue case is not covered
```

## Wildcard patterns

When you want a catch-all, use `_`:

```aivi
fun growLength:Int #length:Int =>
    length
     ||> 1 => 2
     ||> 2 => 3
     ||> 3 => 4
     ||> 4 => 5
     ||> 5 => 6
     ||> _ => 6
```

`_` matches anything and does not bind the value.

## Matching on literal values

You can match on integer and text literals directly:

```aivi
fun fizzBuzz:Text #n:Int =>
    n mod 15 == 0
     ||> True  => "FizzBuzz"
     ||> False =>
        n mod 3 == 0
         ||> True  => "Fizz"
         ||> False =>
            n mod 5 == 0
             ||> True  => "Buzz"
             ||> False => n |> toString
```

## Destructuring product types (records)

You can destructure a record in a pattern arm, binding its fields to names:

```aivi
fun describePoint:Text #point:Vec2 =>
    point
     ||> Vec2 x y => "({x}, {y})"
```

Record patterns work similarly:

```aivi
fun scoreOf:Int #game:Game =>
    game
     ||> { score } => score
```

Here `{ score }` matches any `Game` record and binds the `score` field.

## Matching on data-carrying constructors

When a variant carries data, the pattern binds the inner values:

```aivi
type Maybe A = Some A | None

fun unwrapOr:A #default:A #maybe:Maybe A =>
    maybe
     ||> Some value => value
     ||> None       => default
```

`Some value` binds the wrapped `A` to the name `value` in the body.

## Nested patterns

Patterns can be nested. In the snake game, the step logic matches on a record extracted
from a record:

```aivi
fun runningStep:Game #size:BoardSize #direction:Direction #current:Game =>
    current
     ||> { snake, food, score } => movedGame size direction snake food score
```

The record pattern `{ snake, food, score }` binds three fields of `Game` simultaneously,
without needing intermediate `let` bindings.

## \|\|> vs T\|>/F\|>

Use `\|\|>` when matching on a general sum type or literal. Use `T\|>` / `F\|>` when the
value is already a `Bool` and you want a two-branch conditional:

```aivi
-- Bool, use T|>/F|>:
condition
T|> valueIfTrue
F|> valueIfFalse

-- Sum type with 2+ variants, use ||>:
maybeValue
 ||> Some x => useIt x
 ||> None   => fallback
```

## Summary

- `\|\|>` is the match pipe. Each arm is `\|\|> pattern => body`.
- Matching is exhaustive — every variant must be covered.
- `_` is the wildcard that matches anything.
- Patterns can destructure records and data-carrying constructors.
- Patterns can be nested arbitrarily.

[Next: Signals →](/tour/05-signals)
