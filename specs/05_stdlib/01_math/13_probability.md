# Probability & Distribution Domain

The `Probability` domain gives you tools for **Statistical Distributions** and structured randomness.

Standard `random()` just gives you a boring uniform number between 0 and 1. But reality isn't uniform.
*   Heights of people follow a **Bell Curve** (Normal distribution).
*   Radioactive decay follows a **Poisson** distribution.
*   Success/failure rates follow a **Bernoulli** distribution.

This domain lets you define the *shape* of the chaotic world you want to simulate, and then draw mathematically correct samples from it.

## Overview

<<< ../../snippets/from_md/05_stdlib/01_math/13_probability/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/05_stdlib/01_math/13_probability/block_02.aivi{aivi}

## Domain Definition

<<< ../../snippets/from_md/05_stdlib/01_math/13_probability/block_03.aivi{aivi}

## Helper Functions

| Function | Explanation |
| --- | --- |
| **clamp** p<br><pre><code>`Probability -> Probability`</code></pre> | Bounds `p` into `[0.0, 1.0]`. |
| **bernoulli** p<br><pre><code>`Probability -> Distribution Bool`</code></pre> | Creates a distribution over `Bool` with success probability `p`. |
| **uniform** lo hi<br><pre><code>`Float -> Float -> Distribution Float`</code></pre> | Creates a uniform distribution over `[lo, hi]`. |
| **expectation** dist x<br><pre><code>`Distribution Float -> Float -> Float`</code></pre> | Returns the contribution of `x` to the expected value. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/01_math/13_probability/block_04.aivi{aivi}
