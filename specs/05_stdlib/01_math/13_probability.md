# Probability & Distribution Domain

The `Probability` domain gives you tools for **Statistical Distributions** and structured randomness.

Standard `random()` just gives you a boring uniform number between 0 and 1. But reality isn't uniform.
*   Heights of people follow a **Bell Curve** (Normal distribution).
*   Radioactive decay follows a **Poisson** distribution.
*   Success/failure rates follow a **Bernoulli** distribution.

This domain lets you define the *shape* of the chaotic world you want to simulate, and then draw mathematically correct samples from it.

## Overview

```aivi
use aivi.std.math.probability (Normal, uniform)

// Create a Bell curve centered at 0 with standard deviation of 1
distribution = Normal(0.0, 1.0) 

// Get a random number that fits this curve
// (Most values will be near 0, few will be near -3 or 3)
sample = distribution |> sample()
```

## Performance and Implementation (Rust)

Probability and distribution sampling must use established crates for performance and statistical correctness. Prefer `rand` + `rand_distr` for sampling and `statrs` for distribution math rather than bespoke implementations.

## Features

```aivi
Probability = Float
Distribution a = { pdf: a -> Probability }
```

## Domain Definition

```aivi
domain Probability over Probability = {
  (+) : Probability -> Probability -> Probability
  (-) : Probability -> Probability -> Probability
  (*) : Probability -> Probability -> Probability
}
```

## Helper Functions

```aivi
clamp : Probability -> Probability
clamp p = max 0.0 (min 1.0 p)

bernoulli : Probability -> Distribution Bool
bernoulli p = { pdf: \x -> if x then clamp p else clamp (1.0 - p) }

uniform : Float -> Float -> Distribution Float
uniform lo hi = { pdf: \x -> if x < lo || x > hi then 0.0 else 1.0 / (hi - lo) }

expectation : Distribution Float -> Float -> Float
expectation d x = x * d.pdf x
```

## Usage Examples

```aivi
use aivi.std.probability

p = clamp 0.7
coin = bernoulli p
probHeads = coin.pdf true
```
