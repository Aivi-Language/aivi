# Probability & Distribution Domain

The `Probability` domain gives you tools for **Statistical Distributions** and structured randomness.

Standard `random()` just gives you a boring uniform number between 0 and 1. But reality isn't uniform.
*   Heights of people follow a **Bell Curve** (Normal distribution).
*   Radioactive decay follows a **Poisson** distribution.
*   Success/failure rates follow a **Bernoulli** distribution.

This domain lets you define the *shape* of the chaotic world you want to simulate, and then draw mathematically correct samples from it.

## Overview

```aivi
use aivi.probability (Normal, uniform)

// Create a Bell curve centered at 0 with standard deviation of 1
distribution = Normal(0.0, 1.0) 

// Get a random number that fits this curve
// (Most values will be near 0, few will be near -3 or 3)
sample = distribution |> sample()
```


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

| Function | Explanation |
| --- | --- |
| **clamp** p<br><pre><code>`Probability -> Probability`</code></pre> | Bounds `p` into `[0.0, 1.0]`. |
| **bernoulli** p<br><pre><code>`Probability -> Distribution Bool`</code></pre> | Creates a distribution over `Bool` with success probability `p`. |
| **uniform** lo hi<br><pre><code>`Float -> Float -> Distribution Float`</code></pre> | Creates a uniform distribution over `[lo, hi]`. |
| **expectation** dist x<br><pre><code>`Distribution Float -> Float -> Float`</code></pre> | Returns the contribution of `x` to the expected value. |

## Usage Examples

```aivi
use aivi.probability

p = clamp 0.7
coin = bernoulli p
probHeads = coin.pdf true
```
