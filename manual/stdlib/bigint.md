# aivi.bigint

Pure arbitrary-precision integer operations. The MVP exposes one canonical name per operation.
Parsing, narrowing to `Int`, division, and remainder return `Option` when no value can be produced.

```aivi
use aivi.bigint (
    fromInt
    fromText
    toInt
    toText
    add
    sub
    mul
    div
    mod
    pow
    neg
    abs
    cmp
    eq
    gt
    lt
    zero
    one
    negOne
    isZero
    isPositive
    isNegative
)
```

## Conversion

| Function | Type | Behavior |
| --- | --- | --- |
| `fromInt` | `Int -> BigInt` | Widen an `Int` without loss |
| `fromText` | `Text -> Option BigInt` | Parse a signed decimal integer |
| `toInt` | `BigInt -> Option Int` | Narrow when the value fits in `Int` |
| `toText` | `BigInt -> Text` | Render signed decimal text |

## Arithmetic

| Function | Type | Behavior |
| --- | --- | --- |
| `add` | `BigInt -> BigInt -> BigInt` | Addition |
| `sub` | `BigInt -> BigInt -> BigInt` | Subtraction |
| `mul` | `BigInt -> BigInt -> BigInt` | Multiplication |
| `div` | `BigInt -> BigInt -> Option BigInt` | Truncating division; `None` for a zero divisor |
| `mod` | `BigInt -> BigInt -> Option BigInt` | Remainder; `None` for a zero divisor |
| `pow` | `BigInt -> Int -> BigInt` | Integer power; negative exponents currently act as zero |
| `neg` | `BigInt -> BigInt` | Negation |
| `abs` | `BigInt -> BigInt` | Absolute value |

## Comparison

| Function | Type | Behavior |
| --- | --- | --- |
| `cmp` | `BigInt -> BigInt -> Int` | Return `-1`, `0`, or `1` |
| `eq` | `BigInt -> BigInt -> Bool` | Equality |
| `gt` | `BigInt -> BigInt -> Bool` | Strict greater-than |
| `lt` | `BigInt -> BigInt -> Bool` | Strict less-than |
| `isZero` | `BigInt -> Bool` | Compare with `zero` |
| `isPositive` | `BigInt -> Bool` | Compare above `zero` |
| `isNegative` | `BigInt -> Bool` | Compare below `zero` |

## Constants

| Value | Meaning |
| --- | --- |
| `zero` | `0` |
| `one` | `1` |
| `negOne` | `-1` |

```aivi
use aivi.bigint (
    add
    fromText
    toText
)

type Text -> Text -> Option Text
func combineTotals = left right => (fromText left, fromText right)
 ||> (Some a, Some b) -> Some (toText (add a b))
 ||> _                -> None
```
