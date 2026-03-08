# Probability & Distribution Domain

<!-- quick-info: {"kind":"module","name":"aivi.probability"} -->
The `Probability` domain models probabilities and simple probability distributions as inspectable values.
Use it when you want to talk about a named distribution such as Bernoulli or uniform density, instead of passing around unnamed `Float` values.
<!-- /quick-info -->
<div class="import-badge">use aivi.probability<span class="domain-badge">domain</span></div>

This module is for simulations, experiments, randomized testing, and statistical code where the shape of uncertainty matters. In plain language, the “shape” is the pattern of likely outcomes: equal density across an interval, a weighted coin flip, or some other explicit distribution.

The current API is descriptive rather than effectful: it lets you define distributions and inspect `pdf`, but it does not currently include a sampling function.

## Start here

Choose a named distribution when you want readers to understand *what kind* of uncertainty you mean, not just that “something random” happened. For general numeric helpers around `Float`, see [Math](math.md).

## What it is for

A few examples:

- a coin flip with a configurable chance of success
- a uniform density over a numeric interval
- code that reasons about weights or expected-value contributions, not just raw samples

## Overview

<<< ../../snippets/from_md/stdlib/math/probability/block_01.aivi{aivi}


## Features

<<< ../../snippets/from_md/stdlib/math/probability/block_02.aivi{aivi}


`Probability` is currently just an alias for `Float`, so the type is convenient but not range-safe by itself. Use `clamp` when a computed value may fall outside `[0.0, 1.0]`.

`Distribution A` exposes `pdf`, which is the pointwise probability function for the value type `A`:

- for discrete distributions such as `bernoulli`, `pdf` behaves like probability mass;
- for continuous distributions such as `uniform`, `pdf` behaves like density at a point.

## Domain Definition

<<< ../../snippets/from_md/stdlib/math/probability/block_03.aivi{aivi}


The domain gives you convenient arithmetic on probability-typed values, but it does not enforce normalization or the `[0.0, 1.0]` invariant.

## Core helpers

| Function | What it does |
| --- | --- |
| **clamp** p<br><code>Probability -> Probability</code> | Restricts `p` to the valid probability range `[0.0, 1.0]`. |
| **bernoulli** p<br><code>Probability -> Distribution Bool</code> | Creates a two-outcome distribution that succeeds with probability `p`. The current implementation does not clamp automatically, so pass a value already in `[0.0, 1.0]` or use `clamp` first. |
| **uniform** lo hi<br><code>Float -> Float -> Distribution Float</code> | Creates a uniform density over `[lo, hi]` when `lo < hi`. The current implementation does not reorder or normalize invalid bounds, so equal or reversed bounds do not produce a meaningful uniform distribution. |
| **expectation** dist x<br><code>Distribution Float -> Float -> Float</code> | Returns the point contribution `dist.pdf x * x`. For discrete distributions you sum these contributions across the support; for continuous distributions integration is outside the current API. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/math/probability/block_04.aivi{aivi}


<<< ../../snippets/from_md/stdlib/math/probability/block_05.aivi{aivi}


In the second example, `halfContribution` is only the contribution at `0.5`, not the total expected value of the whole distribution.
