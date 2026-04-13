# aivi.pair

Utilities for working with two-element tuples. Pairs are written `(A, B)` and are the primary way to group two values of potentially different types.

```aivi
use aivi.pair (
    first
    second
    mapFirst
    mapSecond
    fst
    snd
    swap
    mapFst
    mapSnd
    mapBoth
    fromPair
    toPair
    duplicate
)
```

## At a glance

### Canonical pair helpers

| Function | Type | Use it for |
| --- | --- | --- |
| `first` | `(A, B) -> A` | Extract the left item |
| `second` | `(A, B) -> B` | Extract the right item |
| `swap` | `(A, B) -> (B, A)` | Reverse the pair order |
| `mapFirst` | `(A -> C) -> (A, B) -> (C, B)` | Transform only the left item |
| `mapSecond` | `(B -> C) -> (A, B) -> (A, C)` | Transform only the right item |
| `mapBoth` | `(A -> C) -> (B -> D) -> (A, B) -> (C, D)` | Transform both sides with separate functions |
| `fromPair` | `A -> B -> (A, B)` | Build a pair from two values |
| `toPair` | `A -> B -> (A, B)` | Named pairing combinator |
| `duplicate` | `A -> (A, A)` | Put the same value on both sides |

### Compatibility aliases

| Function | Type | Status |
| --- | --- | --- |
| `fst` | `(A, B) -> A` | Compatibility alias for `first` |
| `snd` | `(A, B) -> B` | Compatibility alias for `second` |
| `mapFst` | `(A -> C) -> (A, B) -> (C, B)` | Compatibility alias for `mapFirst` |
| `mapSnd` | `(B -> C) -> (A, B) -> (A, C)` | Compatibility alias for `mapSecond` |

---

## first

Extracts the first element of a pair.

```aivi
type (Text, Int) -> Text
func getKey = entry =>
    first entry
```

---

## second

Extracts the second element of a pair.

```aivi
type (Text, Int) -> Int
func getValue = entry =>
    second entry
```

---

## swap

Swaps the two elements of a pair, returning `(B, A)` from `(A, B)`.

```aivi
use aivi.pair (swap)

type (Text, Int) -> (Int, Text)
func flipEntry = entry =>
    swap entry
```

---

## mapFirst

Applies a function to the first element, leaving the second unchanged.

```aivi
use aivi.math (square)

type (Int, Text) -> (Int, Text)
func squareFst = pair =>
    mapFirst square pair
```

---

## mapSecond

Applies a function to the second element, leaving the first unchanged.

```aivi
use aivi.math (abs)

type (Text, Int) -> (Text, Int)
func absValue = entry =>
    mapSecond abs entry
```

---

## mapBoth

Applies one function to the first element and another to the second.

```aivi
use aivi.pair (mapBoth)

use aivi.math (
    abs
    negate
)

type (Int, Int) -> (Int, Int)
func normalizePair = pair =>
    mapBoth abs negate pair
```

---

## fromPair

Constructs a pair from two separate values.

```aivi
use aivi.pair (fromPair)

type Text -> Int -> (Text, Int)
func makeEntry = label score =>
    fromPair label score
```

---

## toPair

Constructs a pair from two separate values. Useful as a named combinator when pairing results in a pipeline.

```aivi
use aivi.pair (toPair)

type Text -> Int -> (Text, Int)
func labelScore = label score =>
    toPair label score
```

---

## duplicate

Creates a pair where both elements are the same value.

```aivi
use aivi.pair (duplicate)

type Int -> (Int, Int)
func mirror = n =>
    duplicate n
```

---

## Compatibility aliases

`fst`, `snd`, `mapFst`, and `mapSnd` remain available for compatibility, but new code should
prefer `first`, `second`, `mapFirst`, and `mapSecond`.
