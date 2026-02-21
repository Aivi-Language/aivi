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
| **range** start end<br><pre><code>`Int -> Int -> Generator Int`</code></pre> | Produces integers in `[start, end)`. When `end <= start`, it is empty. |
| **fromList** list<br><pre><code>`List a -> Generator a`</code></pre> | Builds a generator from a list. |
| **repeat** value<br><pre><code>`a -> Generator a`</code></pre> | Infinite generator that yields `value` forever. Must be consumed with `take`. |
| **iterate** f seed<br><pre><code>`(a -> a) -> a -> Generator a`</code></pre> | Infinite generator: `seed`, `f seed`, `f (f seed)`, … |
| **unfold** f seed<br><pre><code>`(b -> Option (a, b)) -> b -> Generator a`</code></pre> | Produces elements until `f` returns `None`. |
| **empty**<br><pre><code>`Generator a`</code></pre> | A generator that yields no elements. |

### Transformation

| Function | Explanation |
| --- | --- |
| **map** f gen<br><pre><code>`(a -> b) -> Generator a -> Generator b`</code></pre> | Transforms each element. |
| **filter** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Generator a`</code></pre> | Keeps elements where `pred` holds. |
| **flatMap** f gen<br><pre><code>`(a -> Generator b) -> Generator a -> Generator b`</code></pre> | Maps then flattens (monadic bind). |
| **take** n gen<br><pre><code>`Int -> Generator a -> Generator a`</code></pre> | Takes at most `n` elements. |
| **drop** n gen<br><pre><code>`Int -> Generator a -> Generator a`</code></pre> | Skips the first `n` elements. |
| **takeWhile** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Generator a`</code></pre> | Takes elements while `pred` holds; stops at the first `false`. |
| **dropWhile** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Generator a`</code></pre> | Drops leading elements while `pred` holds. |
| **zip** genA genB<br><pre><code>`Generator a -> Generator b -> Generator (a, b)`</code></pre> | Pairs elements from two generators; stops when either is exhausted. |
| **zipWith** f genA genB<br><pre><code>`(a -> b -> c) -> Generator a -> Generator b -> Generator c`</code></pre> | Combines paired elements with `f`. |
| **enumerate** gen<br><pre><code>`Generator a -> Generator (Int, a)`</code></pre> | Pairs each element with its zero-based index. |
| **scan** f init gen<br><pre><code>`(b -> a -> b) -> b -> Generator a -> Generator b`</code></pre> | Like `foldl` but yields every intermediate accumulator. |
| **concat** genA genB<br><pre><code>`Generator a -> Generator a -> Generator a`</code></pre> | Yields all elements of `genA` followed by all elements of `genB`. |
| **intersperse** sep gen<br><pre><code>`a -> Generator a -> Generator a`</code></pre> | Inserts `sep` between every pair of elements. |
| **chunk** size gen<br><pre><code>`Int -> Generator a -> Generator (List a)`</code></pre> | Groups elements into lists of `size`. The last chunk may be shorter. |
| **dedup** gen<br><pre><code>`Generator a -> Generator a`</code></pre> | Removes consecutive duplicates (requires `Eq`). |

### Consumption

| Function | Explanation |
| --- | --- |
| **foldl** step init gen<br><pre><code>`(b -> a -> b) -> b -> Generator a -> b`</code></pre> | Folds a generator left-to-right. |
| **toList** gen<br><pre><code>`Generator a -> List a`</code></pre> | Materializes a generator into a list. |
| **count** gen<br><pre><code>`Generator a -> Int`</code></pre> | Counts the number of elements (consumes the generator). |
| **any** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Bool`</code></pre> | Returns `true` if any element satisfies `pred` (short-circuits). |
| **all** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Bool`</code></pre> | Returns `true` if every element satisfies `pred` (short-circuits). |
| **find** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Option a`</code></pre> | Returns the first element matching `pred`, or `None`. |
| **head** gen<br><pre><code>`Generator a -> Option a`</code></pre> | Returns the first element, or `None` if empty. |
| **forEach** f gen<br><pre><code>`(a -> Unit) -> Generator a -> Unit`</code></pre> | Applies `f` to each element for side effects. |

Notes:
- Generators are lazy: no work happens until a consumption function is called.
- Infinite generators (`repeat`, `iterate`) must be bounded with `take` or `takeWhile` before materialisation.

