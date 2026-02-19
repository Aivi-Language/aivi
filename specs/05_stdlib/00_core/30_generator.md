# Generator Module

<!-- quick-info: {"kind":"module","name":"aivi.generator"} -->
The `aivi.generator` module provides utilities for AIVI generators.

A generator is a pure, lazy sequence encoded as:

`Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

This makes generators easy to map/filter/fold without loops or mutation.
<!-- /quick-info -->
<div class="import-badge">use aivi.generator</div>


## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/30_generator/block_01.aivi{aivi}

## Type

`Generator A` is a type alias for the core encoding used by `generate { ... }`.

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **foldl** step init gen<br><pre><code>`(b -> a -> b) -> b -> Generator a -> b`</code></pre> | Folds a generator left-to-right. |
| **toList** gen<br><pre><code>`Generator a -> List a`</code></pre> | Materializes a generator into a list. |
| **fromList** list<br><pre><code>`List a -> Generator a`</code></pre> | Builds a generator from a list. |
| **map** f gen<br><pre><code>`(a -> b) -> Generator a -> Generator b`</code></pre> | Transforms elements in a generator. |
| **filter** pred gen<br><pre><code>`(a -> Bool) -> Generator a -> Generator a`</code></pre> | Keeps elements where `pred` holds. |
| **range** start end<br><pre><code>`Int -> Int -> Generator Int`</code></pre> | Produces integers in `[start, end)`. When `end <= start`, it is empty. |

