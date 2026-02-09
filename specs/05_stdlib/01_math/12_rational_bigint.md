# Rational & BigInt Domains

This domain provides **Arbitrary-Precision Integers** (`BigInt`) and **Exact Rational Numbers** (`Rational`).

## What are they?

- **BigInt**: Standard integers (`Int`) are limited to 64 bits (max ~9 quintillion). `BigInt` grows automatically to fit *any* integer number, limited only by your computer's RAM. They are essential for cryptography, combinatorics, and counting things that exceed standard limits.
- **Rational**: Computers usually store fractions as floating-point decimals (`0.1`), which are imprecise approximations. `Rational` numbers store exact fractions (like `1/3`), ensuring that `1/3 + 1/3 + 1/3` equals exactly `1`, not `0.999999`.

## Why this exists

Floating-point math has inherent precision errors (e.g., `0.1 + 0.2 != 0.3` in standard binary math). For financial calculations, scientific proofs, or algorithms requiring exactness, standard floats are dangerous. These types guarantee precision.

## Overview

```aivi
import aivi.std.math.number use { BigInt, Ratio }

// Calculate with atoms in the universe
let huge = 10_000_000_000_000_000_000_000n

// Exact fraction arithmetic
let part = 1/3 + 1/6 
// -> Result is exactly 1/2, not 0.4999...
```

## Features

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