# Probability & Distribution Domain

<!-- quick-info: {"kind":"module","name":"aivi.probability"} -->
The `Probability` domain models probabilities and probability distributions so you can express random choices with explicit intent.
Use it when “pick a random number” is too vague and you want a named distribution such as Bernoulli or uniform sampling.
<!-- /quick-info -->
<div class="import-badge">use aivi.probability<span class="domain-badge">domain</span></div>

This domain is for simulations, experiments, sampling, randomized testing, and statistical code where the shape of randomness matters.

## What it is for

A few examples:

- a coin flip with a configurable chance of success
- a random value drawn from a numeric interval
- code that talks about expected values, not just raw samples

## Overview

<<< ../../snippets/from_md/stdlib/math/probability/overview.aivi{aivi}

## Features

<<< ../../snippets/from_md/stdlib/math/probability/features.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/probability/domain_definition.aivi{aivi}

## Core helpers

| Function | What it does |
| --- | --- |
| **clamp** p<br><code>Probability -> Probability</code> | Restricts `p` to the valid probability range `[0.0, 1.0]`. |
| **bernoulli** p<br><code>Probability -> Distribution Bool</code> | Creates a two-outcome distribution that succeeds with probability `p`. |
| **uniform** lo hi<br><code>Float -> Float -> Distribution Float</code> | Creates a uniform distribution over `[lo, hi]`. |
| **expectation** dist x<br><code>Distribution Float -> Float -> Float</code> | Returns the contribution of `x` to the expected value of `dist`. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/probability/usage_examples.aivi{aivi}
