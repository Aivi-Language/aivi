# aivi.core.fn

Higher-order function combinators. This module provides building blocks for working with functions as first-class values — composing, flipping, and applying them in pipelines.

```aivi
use aivi.core.fn (
    identity
    const
    flip
    compose
    andThen
    on
)
```

## At a glance

| Function | Type | Use it for |
| --- | --- | --- |
| `identity` | `A -> A` | Return a value unchanged |
| `const` | `A -> B -> A` | Ignore the second argument and keep the first |
| `flip` | `(A -> B -> C) -> B -> A -> C` | Reverse the first two arguments of a function |
| `compose` | `(B -> C) -> (A -> B) -> A -> C` | Right-to-left composition |
| `andThen` | `(A -> B) -> (B -> C) -> A -> C` | Left-to-right composition |
| `on` | `(B -> B -> C) -> (A -> B) -> A -> A -> C` | Compare or combine after projecting both sides |

---

## identity

Returns its argument unchanged. Useful as a no-op transformer in pipelines.


```aivi
use aivi.core.fn (identity)

type Int -> Int
func keepAsIs = n =>
    identity n
```

---

## const

Returns a function that always returns its first argument, ignoring the second. Useful for discarding an input in a pipeline step.


```aivi
use aivi.core.fn (const)

type Text -> Int
func alwaysForty = ignored =>
    const 42 ignored
```

---

## flip

Reverses the order of the first two arguments of a two-argument function.


```aivi
use aivi.core.fn (flip)

use aivi.math (clamp)

type Int -> Int -> Int -> Int
func clampFlipped = high low n =>
    flip clamp high low n
```

---

## compose

Composes two functions, applying `g` first and then `f`. `compose f g x` is equivalent to `f (g x)`.


```aivi
use aivi.core.fn (compose)

use aivi.math (
    negate
    abs
)

type Int -> Int
func negAbs = n =>
    compose negate abs n
```

---

## andThen

Applies `f` first and then `g`. The reverse of `compose`. `andThen f g x` is equivalent to `g (f x)`. Often called "left-to-right composition" or `>>>`.


```aivi
use aivi.core.fn (andThen)

use aivi.math (
    abs
    negate
)

type Int -> Int
func absNeg = n =>
    andThen abs negate n
```

---

## on

Applies a transformation `f` to both arguments before combining them with `combine`. Useful for comparing or combining values after mapping.


```aivi
use aivi.core.fn (on)

use aivi.math (abs)

type Int -> Int -> Bool
func byInt = left right =>
    left < right

type Int -> Int -> Bool
func absCompare = x y =>
    on byInt abs x y
```
