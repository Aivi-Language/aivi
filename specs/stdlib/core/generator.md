# Generator Module

<!-- quick-info: {"kind":"module","name":"aivi.generator"} -->
The `aivi.generator` module provides the small core API for working with AIVI generators.

Think of a generator as a **lazy recipe for values**. If you know iterators or streams from other languages, that is the right mental model: a generator describes how to produce items one at a time, and nothing happens until a consumer such as `toList` or `reduce` asks for results.
<!-- /quick-info -->
<div class="import-badge">use aivi.generator</div>

## Start here

If you are new to generators, copy this three-stage workflow first:

1. build a generator,
2. transform it,
3. consume it.

```aivi
numbers = range 1 6
kept    = filter (n => n > 2) numbers
shifted = map (n => n + 1) kept
result  = toList shifted
```

Read that top to bottom as: “describe a sequence, narrow it, adjust it, then finally materialize it.”

## Mental model first

`Generator A` is best understood as “a reusable plan for producing `A` values on demand,” not as “a list that already exists.”

- **construct** a generator with helpers such as `range`, `fromList`, or a [`generate { ... }`](../../syntax/generators.md) block
- **transform** it with helpers such as `map` and `filter`
- **consume** it with helpers such as `toList` and `reduce`

### Optional deep dive: representation

The underlying encoding is:

`Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

You do not need that type equation for everyday use. In plain language, it means a generator can feed values into whatever consumer you provide without exposing loops or mutable iteration state. Another way to say it: a generator is a value-producing plan that already knows how to cooperate with folds.

## What generators are for

Use a generator when you want to describe a sequence of values without building the whole sequence in memory up front. That is especially helpful for:

- stepping through numeric ranges,
- streaming transformed data through a pipeline,
- keeping sequence-building logic pure and reusable,
- writing sequence logic with [`generate { ... }`](../../syntax/generators.md) when nested loops, guards, or `loop` / `recurse` make the intent clearer.

A generator stays **lazy** until you consume it.

## Overview

<<< ../../snippets/from_md/stdlib/core/generator/overview.aivi{aivi}

## The `Generator A` type

`Generator A` is the type behind `generate { ... }`. If you have used iterators or streams in other languages, the idea is similar: it is a reusable description of how to produce values one at a time.

Because generators are pure values, you can pass them around, transform them, and combine them without hidden mutation.

The `aivi.generator` module keeps the v0.1 surface intentionally small: construction, mapping / filtering, and consumption. For richer sequence-building syntax, see [Generator expressions](../../syntax/generators.md). When you need list-oriented helpers such as `take`, `zip`, `chunk`, or indexing, convert with `toList` and continue with the APIs documented in [Collections](collections.md).

## Core API (v0.1)

The exported names line up with the shared `Functor`, `Filterable`, and `Foldable` vocabulary from [aivi.logic](logic.md).

### Construction

These helpers answer “where do the values come from?”.

| Function | Explanation |
| --- | --- |
| **range** start end<br><code>Int -> Int -> Generator Int</code> | Produces integers in `[start, end)`. When `end <= start`, the generator is empty. |
| **fromList** list<br><code>List a -> Generator a</code> | Turns an existing list into a generator. |

### Transformation

Use this group when you want to reshape a sequence without forcing it into memory.

| Function | Explanation |
| --- | --- |
| **map** f gen<br><code>(a -> b) -> Generator a -> Generator b</code> | Transforms each produced value. |
| **filter** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Keeps only values that satisfy `pred`. |

### Consumption

These helpers are the finish line: they answer “what final result do I want from this generator?”.

| Function | Explanation |
| --- | --- |
| **reduce** step init gen<br><code>(b -> a -> b) -> b -> Generator a -> b</code> | Consumes the generator left to right and accumulates a final value. This is the generator-facing `Foldable` operation. |
| **toList** gen<br><code>Generator a -> List a</code> | Materializes the generator into a list. |

### A small `reduce` example

`reduce` is the clearest way to summarize a generator into one value:

```aivi
numbers = range 1 5
sum     = reduce (total => n => total + n) 0 numbers
```

The final `sum` is `10`, because `range 1 5` produces `[1, 2, 3, 4]`.

## Practical guidance

- Prefer generators when you want pipeline-friendly, single-pass processing.
- Prefer [`generate { ... }`](../../syntax/generators.md) when nested loops, guards, or tail-recursive `loop` / `recurse` express the sequence more clearly than plain mapping and filtering.
- Prefer lists once you need helpers from [`aivi.list`](collections.md), such as `take`, `zip`, `chunk`, or `at`.
- `toList` is the usual boundary between a lazy sequence description and a concrete collection.
- For summary values, `reduce` is usually clearer than calling the Church-encoded generator directly.
