# aivi.core.range

`aivi.core.range` provides closed numeric intervals. Bounds are inclusive. An integer or floating-point range is empty when `start > end`; constructors preserve the supplied bounds rather than reordering them.

## Integer ranges

| Export | Type | Behavior |
| --- | --- | --- |
| `RangeInt` | `{ start: Int, end: Int }` | Inclusive integer range |
| `make` | `Int -> Int -> RangeInt` | Construct a range |
| `isEmpty` | `RangeInt -> Bool` | Test whether `start > end` |
| `contains` | `RangeInt -> Int -> Bool` | Test inclusive membership |
| `length` | `RangeInt -> Int` | Count integers; return zero for an empty range |
| `overlaps` | `RangeInt -> RangeInt -> Bool` | Test whether two nonempty ranges share a value |
| `clampTo` | `RangeInt -> Int -> Int` | Restrict a value to the two bounds |
| `startOf` | `RangeInt -> Int` | Read the start bound |
| `endOf` | `RangeInt -> Int` | Read the end bound |
| `shift` | `Int -> RangeInt -> RangeInt` | Add a delta to both bounds |
| `intersect` | `RangeInt -> RangeInt -> RangeInt` | Return the shared interval, possibly empty |

## Floating-point ranges

| Export | Type | Behavior |
| --- | --- | --- |
| `RangeFloat` | `{ start: Float, end: Float }` | Inclusive floating-point range |
| `makeFloat` | `Float -> Float -> RangeFloat` | Construct a range |
| `isEmptyFloat` | `RangeFloat -> Bool` | Test whether `start > end` |
| `containsFloat` | `RangeFloat -> Float -> Bool` | Test inclusive membership |
| `clampToFloat` | `RangeFloat -> Float -> Float` | Restrict a value to the two bounds |
| `shiftFloat` | `Float -> RangeFloat -> RangeFloat` | Add a delta to both bounds |
| `lerpFloat` | `RangeFloat -> Float -> Float` | Interpolate by `t`; values outside zero through one extrapolate |

```aivi
use aivi.core.range (
    RangeInt
    contains
    length
    make
)

value pageWindow : RangeInt = make 10 19
value includesLast : Bool = contains pageWindow 19
value pageSize : Int = length pageWindow
```
