# aivi.core.range

Integer range utilities with an inclusive `[start, end]` interval type. All operations are pure AIVI — no I/O, no intrinsics.

```aivi
use aivi.core.range (
    RangeInt
    make
    isEmpty
    contains
    length
    overlaps
    clampTo
    startOf
    endOf
    shift
    intersect
)
```

---

## Type

### `RangeInt`

```aivi
type RangeInt = {
    start: Int,
    end: Int
}
```

An inclusive integer range. A range where `start > end` is considered **empty**.

---

## Construction

### `make : Int -> Int -> RangeInt`

Create a range from `start` to `end` (inclusive).

```aivi
use aivi.core.range (make)

value r = make 1 10
```

---

## Querying

### `isEmpty : RangeInt -> Bool`

Returns `True` when `start > end`.

```aivi
# <unparseable item>
```

### `contains : RangeInt -> Int -> Bool`

Returns `True` when `n` is within the range (inclusive on both ends).

```aivi
# <unparseable item>
```

### `length : RangeInt -> Int`

Returns the number of integers in the range. Empty ranges have length `0`.

```aivi
# <unparseable item>
```

### `startOf : RangeInt -> Int` / `endOf : RangeInt -> Int`

Extract the start or end bound.

```aivi
# <unparseable item>
```

---

## Operations

### `clampTo : RangeInt -> Int -> Int`

Clamp a value to the range boundaries.

```aivi
# <unparseable item>
```

### `shift : Int -> RangeInt -> RangeInt`

Translate the entire range by a delta.

```aivi
# <unparseable item>
```

### `overlaps : RangeInt -> RangeInt -> Bool`

Returns `True` when two ranges share at least one integer. Empty ranges never overlap.

```aivi
# <unparseable item>
```

### `intersect : RangeInt -> RangeInt -> RangeInt`

Returns the intersection of two ranges. If the ranges do not overlap the result is an empty range (`start > end`).

```aivi
# <unparseable item>
```

---

## Real-world example

```aivi
use aivi.core.range (
    make
    contains
    clampTo
    length
)

type Viewport = {
    visible: RangeInt,
    total: Int
}

fun visibleRows: (List Int) viewport:Viewport rows: (List Int) => rows
  |> filter (contains viewport.visible)

fun scrollProgress:Float viewport:Viewport => viewport.visible.start
  |> toFloat
  |> divide (toFloat viewport.total)
```
