# Generator Module

<!-- quick-info: {"kind":"module","name":"aivi.generator"} -->
The `aivi.generator` module provides utilities for AIVI generators.

Think of a generator as a **lazy recipe for values**. If you know iterators or streams from other languages, that is the right mental model: the generator describes how to produce items one at a time, and nothing happens until a consumer asks for them.
<!-- /quick-info -->
<div class="import-badge">use aivi.generator</div>

## Start here

If you are new to generators, copy this three-stage workflow first:

1. build a generator,
2. transform it,
3. consume it.

```aivi
numbers    = range 1 10
shifted    = map (n => n + 1) numbers
firstThree = take 3 shifted
result     = toList firstThree
```

Read that top to bottom as: “describe a sequence, adjust it, then finally materialize it.”

## Mental model first

`Generator A` is best understood as “a reusable plan for producing `A` values on demand,” not as “a list that already exists.”

- **construct** a generator with helpers such as `range`, `fromList`, or `iterate`
- **transform** it with helpers such as `map`, `filter`, or `zip`
- **consume** it with helpers such as `toList`, `foldl`, `find`, or `count`

### Optional deep dive: representation

The underlying encoding is:

`Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

You do not need that type equation for everyday use. In plain language, it means a generator can feed values into whatever consumer you provide without exposing loops or mutable iteration state. Another way to say it: a generator is a value-producing plan that already knows how to cooperate with folds.

## What generators are for

Use a generator when you want to describe a sequence of values without building the whole sequence in memory up front. That is especially helpful for:

- stepping through numeric ranges,
- streaming transformed data through a pipeline,
- working with large or even infinite sequences,
- stopping early with consumers such as `find`, `any`, or `take`.

A generator stays **lazy** until you consume it.

## Overview

<<< ../../snippets/from_md/stdlib/core/generator/overview.aivi{aivi}

## The `Generator A` type

`Generator A` is the type behind `generate { ... }`. If you have used iterators or streams in other languages, the idea is similar: it is a reusable description of how to produce values one at a time.

Because generators are pure values, you can pass them around, transform them, and combine them without hidden mutation.

## Core API (v0.1)

### Construction

These helpers answer “where do the values come from?”.

| Function | Explanation |
| --- | --- |
| **range** start end<br><code>Int -> Int -> Generator Int</code> | Produces integers in `[start, end)`. When `end <= start`, the generator is empty. |
| **fromList** list<br><code>List a -> Generator a</code> | Turns an existing list into a generator. |
| **repeat** value<br><code>a -> Generator a</code> | Produces `value` forever. Pair it with `take` or another bounded consumer. |
| **iterate** f seed<br><code>(a -> a) -> a -> Generator a</code> | Produces `seed`, then `f seed`, then `f (f seed)`, and so on forever. |
| **unfold** f seed<br><code>(b -> Option (a, b)) -> b -> Generator a</code> | Builds a generator by repeatedly asking `f` for the next item and next seed. Returning `None` stops the sequence. |
| **empty**<br><code>Generator a</code> | A generator that yields nothing. |

### Transformation

Use this group when you want to reshape a sequence without forcing it into memory. A simple mental model:

- `map`, `filter`, `take`, `drop` reshape one generator
- `zip`, `zipWith`, `concat`, `intersperse` combine or coordinate generators
- `scan`, `chunk`, `enumerate`, `dedup` add structure to the stream of values

| Function | Explanation |
| --- | --- |
| **map** f gen<br><code>(a -> b) -> Generator a -> Generator b</code> | Transforms each produced value. |
| **filter** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Keeps only values that satisfy `pred`. |
| **flatMap** f gen<br><code>(a -> Generator b) -> Generator a -> Generator b</code> | Replaces each value with another generator and flattens the result. |
| **take** n gen<br><code>Int -> Generator a -> Generator a</code> | Keeps at most the first `n` values. |
| **drop** n gen<br><code>Int -> Generator a -> Generator a</code> | Skips the first `n` values. |
| **takeWhile** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Keeps values until `pred` becomes false. |
| **dropWhile** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Skips leading values while `pred` is true. |
| **zip** genA genB<br><code>Generator a -> Generator b -> Generator (a, b)</code> | Pairs values from two generators and stops when either side ends. |
| **zipWith** f genA genB<br><code>(a -> b -> c) -> Generator a -> Generator b -> Generator c</code> | Zips and combines in one step. |
| **enumerate** gen<br><code>Generator a -> Generator (Int, a)</code> | Adds a zero-based index to each value. |
| **scan** f init gen<br><code>(b -> a -> b) -> b -> Generator a -> Generator b</code> | Like a running fold: it yields every intermediate accumulator, which is useful for running totals or progressive summaries. |
| **concat** genA genB<br><code>Generator a -> Generator a -> Generator a</code> | Produces all of `genA`, then all of `genB`. |
| **intersperse** sep gen<br><code>a -> Generator a -> Generator a</code> | Inserts `sep` between yielded values. |
| **chunk** size gen<br><code>Int -> Generator a -> Generator (List a)</code> | Groups values into lists of size `size`. The final chunk may be shorter. |
| **dedup** gen<br><code>Generator a -> Generator a</code> | Removes consecutive duplicate values. |

### Consumption

These helpers are the finish line: they answer “what final result do I want from this generator?”.

| Function | Explanation |
| --- | --- |
| **foldl** step init gen<br><code>(b -> a -> b) -> b -> Generator a -> b</code> | Consumes the generator left to right and accumulates a final value. |
| **toList** gen<br><code>Generator a -> List a</code> | Materializes the generator into a list. |
| **count** gen<br><code>Generator a -> Int</code> | Counts how many values are produced. |
| **any** pred gen<br><code>(a -> Bool) -> Generator a -> Bool</code> | Returns as soon as it sees a matching value. |
| **all** pred gen<br><code>(a -> Bool) -> Generator a -> Bool</code> | Returns as soon as it sees a value that fails the predicate. |
| **find** pred gen<br><code>(a -> Bool) -> Generator a -> Option a</code> | Returns the first matching value, or `None`. |
| **head** gen<br><code>Generator a -> Option a</code> | Returns the first value, or `None` if the generator is empty. |
| **forEach** f gen<br><code>(a -> Unit) -> Generator a -> Unit</code> | Runs `f` for every yielded value when you need side effects. |

### A small `scan` example

`scan` is easiest to understand by looking at the intermediate states it keeps:

```aivi
numbers       = fromList [1, 2, 3]
runningTotals = scan (total => n => total + n) 0 numbers
result        = toList runningTotals
```

The final `result` is `[0, 1, 3, 6]`, because `scan` keeps every accumulator value instead of only the last one.

## Practical guidance

- Prefer generators when you want pipeline-friendly, lazy processing.
- Prefer lists when you already need all values in memory or want random access helpers such as `at`.
- Infinite generators such as `repeat` and `iterate` are safe, but only when a later step limits them with `take`, `takeWhile`, `find`, or a similar consumer.
- `toList` is the usual “finish line” when you want a concrete collection at the end of the pipeline.
