# Generator Module

<!-- quick-info: {"kind":"module","name":"aivi.generator"} -->
The `aivi.generator` module provides utilities for AIVI generators.

Think of a generator as a **lazy recipe for values**. If you know iterators or streams from other languages, that is the right mental model: the generator describes how to produce items one at a time, and nothing happens until a consumer asks for them.
<!-- /quick-info -->
<div class="import-badge">use aivi.generator</div>

## Mental model first

`Generator A` is best understood as “a reusable plan for producing `A` values on demand,” not as “a list that already exists.”

- **construct** a generator with helpers such as `range`, `fromList`, or `iterate`
- **transform** it with helpers such as `map`, `filter`, or `zip`
- **consume** it with helpers such as `toList`, `foldl`, `find`, or `count`

### Advanced note: representation

The underlying encoding is:

`Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

You do not need that type equation for everyday use. It means a generator can feed its values into any fold-like consumer without exposing loops or mutable iteration state.

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

| Function | Explanation |
| --- | --- |
| **range** start end<br><code>Int -> Int -> Generator Int</code> | Produces integers in `[start, end)`. When `end <= start`, the generator is empty. |
| **fromList** list<br><code>List a -> Generator a</code> | Turns an existing list into a generator. |
| **repeat** value<br><code>a -> Generator a</code> | Produces `value` forever. Pair it with `take` or another bounded consumer. |
| **iterate** f seed<br><code>(a -> a) -> a -> Generator a</code> | Produces `seed`, then `f seed`, then `f (f seed)`, and so on forever. |
| **unfold** f seed<br><code>(b -> Option (a, b)) -> b -> Generator a</code> | Builds a generator by repeatedly asking `f` for the next item and next seed. Returning `None` stops the sequence. |
| **empty**<br><code>Generator a</code> | A generator that yields nothing. |

### Transformation

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
| **scan** f init gen<br><code>(b -> a -> b) -> b -> Generator a -> Generator b</code> | Like a running fold: it yields every intermediate accumulator. |
| **concat** genA genB<br><code>Generator a -> Generator a -> Generator a</code> | Produces all of `genA`, then all of `genB`. |
| **intersperse** sep gen<br><code>a -> Generator a -> Generator a</code> | Inserts `sep` between yielded values. |
| **chunk** size gen<br><code>Int -> Generator a -> Generator (List a)</code> | Groups values into lists of size `size`. The final chunk may be shorter. |
| **dedup** gen<br><code>Generator a -> Generator a</code> | Removes consecutive duplicate values. |

### Consumption

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

## Practical guidance

- Prefer generators when you want pipeline-friendly, lazy processing.
- Prefer lists when you already need all values in memory or want random access helpers such as `at`.
- Infinite generators such as `repeat` and `iterate` are safe, but only when a later step limits them with `take`, `takeWhile`, `find`, or a similar consumer.
- `toList` is the usual “finish line” when you want a concrete collection at the end of the pipeline.
