# Fan-out and Collection Shaping

AIVI v0.2 uses flat flow syntax for zero-many workflows. The main tool is `*|>`, which fans out over an iterable, runs a per-item fan-out body, and rejoins with an ordinary list result at `*-|`.

There is no separate legacy generator-block surface in the current language.

## The basic shape

```aivi
evens =
  [1..10]
     *|> _
     >|> _ % 2 == 0
      |> _ * 2
     *-|
```

Read that as:

1. start from an iterable,
2. enter a per-item fan-out body with `*|>`,
3. keep only items whose guard passes,
4. emit the transformed value for each surviving item,
5. rejoin as a normal list at `*-|`.

## What `*|>` does

Inside a fan-out block:

- each item becomes the current spine subject,
- ordinary `|>` lines transform that item,
- `>|>` without `or fail ...` means **skip this item**,
- bindings created inside the fan-out body stay local to that item,
- the whole block yields a plain list value once `*-|` closes it.

That final point matters: after the block finishes, use ordinary collection helpers rather than a second special workflow syntax.

## Shape the result with ordinary functions

```aivi
activeUserIds =
  users
     *|> _
     >|> active
      |> .id
     *-|
      |> toSet
```

Collection shaping stays in regular functions such as `map`, `filter`, `partition`, `groupBy`, `toSet`, and `fold`.

## Nesting fan-out

Nested `*|>` blocks express zero-many combinations without introducing a separate comprehension syntax.

```aivi
grid =
  xs
     *|> _ #x
      |> ys
     *|> _ #y
      |> (x, y)
     *-|
     *-|
```

Use helper functions when nesting stops being readable.

## Concurrency

`*|>` supports `@concurrent` when the runtime may process item fan-out bodies in parallel:

```aivi
savedIds =
  users
     *|> _ @concurrent 8
      |> normalizeUser
      |> saveUser
      |> .id
     *-|
```

`@concurrent` limits how many item fan-out bodies may run at once. It does not change the logical result shape.

## Relationship to `aivi.generator`

The [`aivi.generator`](../stdlib/core/generator.md) module still documents reusable lazy generator values as library data. The language-level workflow surface for zero-many processing, though, is `*|>` plus ordinary collection functions.

## When to use fan-out versus plain collection helpers

Use `*|>` when the sequence logic itself needs workflow structure, for example:

- guards that should skip items,
- per-item fallible or effectful steps,
- nested zero-many expansions,
- scoped bindings that make the item pipeline easier to read.

Use plain `map`, `filter`, and `fold` when a single expression already says the whole transformation clearly.
