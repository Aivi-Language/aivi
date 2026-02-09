# Standard Library: Rational & BigInt Domains

## Module

```aivi
module aivi.std.rational = {
  export domain BigInt
  export domain Rational
  export BigInt, Rational
  export fromInt, normalize, gcd, toFloat
}
```

## Types

```aivi
BigInt = { sign: Int, limbs: List Int }
Rational = { num: BigInt, den: BigInt }
```

## Domain Definition

```aivi
domain BigInt over BigInt = {
  (+) : BigInt -> BigInt -> BigInt
  (-) : BigInt -> BigInt -> BigInt
  (*) : BigInt -> BigInt -> BigInt
}

domain Rational over Rational = {
  (+) : Rational -> Rational -> Rational
  (-) : Rational -> Rational -> Rational
  (*) : Rational -> Rational -> Rational
  (/) : Rational -> Rational -> Rational
}
```

## Helper Functions

```aivi
fromInt : Int -> BigInt
fromInt n = { sign: if n < 0 then -1 else 1, limbs: [abs n] }

gcd : BigInt -> BigInt -> BigInt
gcd a b = if b == fromInt 0 then a else gcd b (a % b)

normalize : Rational -> Rational
normalize r = {
  num: r.num / gcd r.num r.den,
  den: r.den / gcd r.num r.den
}

toFloat : Rational -> Float
toFloat r = (toFloat r.num) / (toFloat r.den)
```

## Usage Examples

```aivi
use aivi.std.rational

half = normalize { num: fromInt 1, den: fromInt 2 }
sum = half + half
```