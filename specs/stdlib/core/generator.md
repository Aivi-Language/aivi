# Generator Module

<!-- quick-info: {"kind":"module","name":"aivi.generator"} -->
The `aivi.generator` module provides utilities for AIVI generators.

A generator is a pure, lazy sequence encoded as:

`Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

This makes generators easy to map/filter/fold without loops or mutation.
<!-- /quick-info -->
<div class="import-badge">use aivi.generator</div>


## Overview

<<< ../../snippets/from_md/stdlib/core/generator/overview.aivi{aivi}

## Type

`Generator A` is a type alias for the core encoding used by `generate { ... }`.

## Core API (v0.1)

### Construction

| Function | Explanation |
| --- | --- |
| **range** start end<br><code>Int -> Int -> Generator Int</code> | Produces integers in `[start, end)`. When `end <= start`, it is empty. |
| **fromList** list<br><code>List a -> Generator a</code> | Builds a generator from a list. |
| **repeat** value<br><code>a -> Generator a</code> | Infinite generator that yields `value` forever. Must be consumed with `take`. |
| **iterate** f seed<br><code>(a -> a) -> a -> Generator a</code> | Infinite generator: `seed`, `f seed`, `f (f seed)`, … |
| **unfold** f seed<br><code>(b -> Option (a, b)) -> b -> Generator a</code> | Produces elements until `f` returns `None`. |
| **empty**<br><code>Generator a</code> | A generator that yields no elements. |

### Transformation

| Function | Explanation |
| --- | --- |
| **map** f gen<br><code>(a -> b) -> Generator a -> Generator b</code> | Transforms each element. |
| **filter** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Keeps elements where `pred` holds. |
| **flatMap** f gen<br><code>(a -> Generator b) -> Generator a -> Generator b</code> | Maps then flattens (monadic bind). |
| **take** n gen<br><code>Int -> Generator a -> Generator a</code> | Takes at most `n` elements. |
| **drop** n gen<br><code>Int -> Generator a -> Generator a</code> | Skips the first `n` elements. |
| **takeWhile** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Takes elements while `pred` holds; stops at the first `false`. |
| **dropWhile** pred gen<br><code>(a -> Bool) -> Generator a -> Generator a</code> | Drops leading elements while `pred` holds. |
| **zip** genA genB<br><code>Generator a -> Generator b -> Generator (a, b)</code> | Pairs elements from two generators; stops when either is exhausted. |
| **zipWith** f genA genB<br><code>(a -> b -> c) -> Generator a -> Generator b -> Generator c</code> | Combines paired elements with `f`. |
| **enumerate** gen<br><code>Generator a -> Generator (Int, a)</code> | Pairs each element with its zero-based index. |
| **scan** f init gen<br><code>(b -> a -> b) -> b -> Generator a -> Generator b</code> | Like `foldl` but yields every intermediate accumulator. |
| **concat** genA genB<br><code>Generator a -> Generator a -> Generator a</code> | Yields all elements of `genA` followed by all elements of `genB`. |
| **intersperse** sep gen<br><code>a -> Generator a -> Generator a</code> | Inserts `sep` between every pair of elements. |
| **chunk** size gen<br><code>Int -> Generator a -> Generator (List a)</code> | Groups elements into lists of `size`. The last chunk may be shorter. |
| **dedup** gen<br><code>Generator a -> Generator a</code> | Removes consecutive duplicates (requires `Eq`). |

### Consumption

| Function | Explanation |
| --- | --- |
| **foldl** step init gen<br><code>(b -> a -> b) -> b -> Generator a -> b</code> | Folds a generator left-to-right. |
| **toList** gen<br><code>Generator a -> List a</code> | Materializes a generator into a list. |
| **count** gen<br><code>Generator a -> Int</code> | Counts the number of elements (consumes the generator). |
| **any** pred gen<br><code>(a -> Bool) -> Generator a -> Bool</code> | Returns `true` if any element satisfies `pred` (short-circuits). |
| **all** pred gen<br><code>(a -> Bool) -> Generator a -> Bool</code> | Returns `true` if every element satisfies `pred` (short-circuits). |
| **find** pred gen<br><code>(a -> Bool) -> Generator a -> Option a</code> | Returns the first element matching `pred`, or `None`. |
| **head** gen<br><code>Generator a -> Option a</code> | Returns the first element, or `None` if empty. |
| **forEach** f gen<br><code>(a -> Unit) -> Generator a -> Unit</code> | Applies `f` to each element for side effects. |

Notes:
- Generators are lazy: no work happens until a consumption function is called.
- Infinite generators (`repeat`, `iterate`) must be bounded with `take` or `takeWhile` before materialisation.

