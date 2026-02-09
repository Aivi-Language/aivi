# Probability & Distribution Domain

A library for **Statistical Distributions** and controlled randomness.

## What is this?

Standard random functions usually just give you a number between 0 and 1. But real-world randomness is rarely that uniform. 
*   Heights of people follow a **Normal (Bell Curve)** distribution.
*   Radioactive decay follows a **Poisson** distribution.
*   Coin flips follow a **Bernoulli** distribution.

This domain lets you define the *shape* of randomness you want, and then draw samples from it.

## Why this exists

Simulations, games, and scientific models need "textured" randomness. Instead of writing complex math to convert a simple random number into a Bell curve manually, you can just ask for a `Normal(mean, std_dev)` distribution and sample it.

## Overview

```aivi
import aivi.std.math.probability use { Normal, uniform }

// Create a Bell curve centered at 0 with standard deviation of 1
let distribution = Normal(0.0, 1.0) 

// Get a random number that fits this curve
// (Most values will be near 0, few will be near -3 or 3)
let sample = distribution |> sample()
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