# aivi.math

Pure integer helpers for sign, bounds, divisibility, and small whole-number calculations.

```aivi
use aivi.math (
    abs
    negate
    sign
    isEven
    isOdd
    square
    clamp
    between
    divides
    gcd
    lcm
    pow
)
```

| Function | Type | Behavior |
| --- | --- | --- |
| `abs` | `Int -> Int` | Absolute value |
| `negate` | `Int -> Int` | Flip the sign |
| `sign` | `Int -> Int` | Return `-1`, `0`, or `1` |
| `isEven` | `Int -> Bool` | Test divisibility by two |
| `isOdd` | `Int -> Bool` | Test non-divisibility by two |
| `square` | `Int -> Int` | Multiply a value by itself |
| `clamp` | `Int -> Int -> Int -> Int` | Restrict a value to inclusive bounds |
| `between` | `Int -> Int -> Int -> Bool` | Test inclusive bounds |
| `divides` | `Int -> Int -> Bool` | Test exact divisibility; zero divides only zero |
| `gcd` | `Int -> Int -> Int` | Greatest common divisor |
| `lcm` | `Int -> Int -> Int` | Least common multiple; returns zero when either input is zero |
| `pow` | `Int -> Int -> Int` | Non-negative integer exponentiation; negative exponents return zero |

`Int` arithmetic has the same fixed-width overflow behavior as ordinary language arithmetic.

```aivi
use aivi.math (
    between
    gcd
    isEven
)

value aligned : Bool = isEven 42
value divisor : Int = gcd 84 30
value validPercent : Bool = between 0 100 75
```
