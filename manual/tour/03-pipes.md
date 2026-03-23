# Pipes

Pipes are the centrepiece of AIVI's surface syntax.
The idea is borrowed from Unix: data flows from left to right through a sequence of transformations.

## The transform pipe \|>

`\|>` takes the value on its left and passes it as the first argument to the function on its right.

```aivi
val result =
    42
     |> double
     |> toString
```

This is equivalent to `toString (double 42)`. Pipes let you read computation top-to-bottom
instead of inside-out.

Compare these two forms of the same computation:

```aivi
-- nested calls (read inside-out)
val result = formatScore (addBonus (clamp 0 100 rawScore) 10)

-- pipes (read top-to-bottom)
val result =
    rawScore
     |> clamp 0 100
     |> addBonus 10
     |> formatScore
```

When you pass partial arguments before the piped value, `\|>` inserts the left-hand value
as the **last** argument:

```aivi
fun clamp:Int #lo:Int #hi:Int #value:Int => ...

val clamped =
    rawScore
     |> clamp 0 100    -- equivalent to: clamp 0 100 rawScore
```

## Projection shorthand

A common pattern is projecting a field from a record:

```aivi
val name = user |> .username
```

The `.field` syntax is a shorthand for `\r => r.field`.
It composes naturally in pipes:

```aivi
sig boardTitle : Signal Text =
    board
     |> .width
     |> \w => "Board width: {w}"
```

## Chaining pipes

Pipes chain arbitrarily. Each `\|>` is one step in the computation:

```aivi
sig scoreLabel : Signal Text =
    game
     |> .score
     |> \n => n * 10
     |> \n => "Score: {n} pts"
```

## Why pipes instead of nested calls?

Consider a computation with five steps. With nested calls:

```aivi
val result = step5 (step4 (step3 (step2 (step1 input))))
```

You must read from the inside out, matching parentheses as you go.

With pipes:

```aivi
val result =
    input
     |> step1
     |> step2
     |> step3
     |> step4
     |> step5
```

The computation reads in execution order, top to bottom.
Each step is on its own line. Inserting, removing, or reordering steps is straightforward.

## The gate pipe ?\|>

`?\|>` passes the value only if a condition is true.
If the condition is false, the value is **suppressed** — nothing flows downstream.

```aivi
sig validInput : Signal Text =
    rawInput
     ?|> \t => t != ""
```

`validInput` only has a value when `rawInput` is non-empty.
This is useful for validation: downstream signals only fire when the gate is open.

```aivi
sig submittable : Signal Form =
    formData
     ?|> \f => f.name != ""
     ?|> \f => f.email != ""
```

## The truthy and falsy pipes T\|> and F\|>

`T\|>` and `F\|>` are conditional path selectors. Given a `Bool` on the left, they pass
a value (not the condition) depending on whether it is `True` or `False`:

```aivi
fun absolute:Int #n:Int =>
    n < 0
    T|> n * (-1)
    F|> n
```

`T\|>` and `F\|>` are usually used in pairs. They are the AIVI alternative to `if`/`else`:

```aivi
fun applyDirection:Direction #current:Direction #candidate:Direction =>
    isOpposite candidate current
    T|> current
    F|> candidate
```

If `isOpposite candidate current` is `True`, the result is `current`.
Otherwise it is `candidate`.

## Operator quick reference

::: details Pipe operator quick reference

| Operator | Name | Reads as |
|---|---|---|
| `\|>` | transform | "then apply" |
| `?\|>` | gate | "only if" |
| `\|\|>` | case | "match against" — see next chapter |
| `*\|>` | map | "for each item in list, apply" |
| `&\|>` | apply | "zip-apply across signals" |
| `T\|>` | truthy branch | "if true, use" |
| `F\|>` | falsy branch | "if false, use" |
| `@\|>` | recur start | "starting from, fold over time" |
| `<\|@` | recur step | "on each event, update with" |
| `<\|*` | fan-in | "merge multiple lists into one" |

:::

## Summary

- `\|>` passes a value through a function, left-to-right.
- `.field` is shorthand for `\r => r.field`.
- Pipes chain: each `\|>` is one step.
- `?\|>` gates: value passes only when the predicate is true.
- `T\|>` and `F\|>` select branches based on a `Bool`.

[Next: Pattern Matching →](/tour/04-pattern-matching)
