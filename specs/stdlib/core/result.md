# Standard Library: Result Utilities

<!-- quick-info: {"kind":"module","name":"aivi.result"} -->
The `aivi.result` module provides utility functions for working with `Result E A` values. These complement the shared `map`/`flatMap` style operations from `aivi.logic` and the `attempt` effect operator.
<!-- /quick-info -->

<div class="import-badge">use aivi.result</div>

## What `Result` means

`Result E A` represents a computation that can succeed or fail.

- `Ok x` holds a successful value.
- `Err e` holds an error value.

Use `Result` when failure is expected and you want to keep it explicit instead of throwing exceptions or hiding error cases.

## Quick chooser

- use `Option` when you only care whether a value exists
- use `Result` when you want one explicit success-or-error path
- use `Validation` when you want to keep going and collect several errors

## Overview

This module provides small helpers for checking results, transforming either side, converting to or from related types, and choosing fallbacks.

## Predicates

| Function | Type | Description |
|----------|------|-------------|
| `isOk` | `Result E A -> Bool` | Returns whether the result is successful |
| `isErr` | `Result E A -> Bool` | Returns whether the result is an error |

<<< ../../snippets/from_md/stdlib/core/result/predicates.aivi{aivi}

## Extracting values

| Function | Type | Description |
|----------|------|-------------|
| `getOrElse` | `A -> Result E A -> A` | Returns the success value, or a default when the result is `Err` |
| `getOrElseLazy` | `(E -> A) -> Result E A -> A` | Returns the success value, or computes a fallback from the error |

<<< ../../snippets/from_md/stdlib/core/result/extracting.aivi{aivi}

`getOrElseLazy` is handy when the fallback depends on the error details.

## Transformations

| Function | Type | Description |
|----------|------|-------------|
| `map` | `(A -> B) -> Result E A -> Result E B` | Transforms the success value while leaving errors alone |
| `mapErr` | `(E -> F) -> Result E A -> Result F A` | Transforms the error value while leaving successes alone |
| `flatMap` | `(A -> Result E B) -> Result E A -> Result E B` | Chains fallible operations together |

<<< ../../snippets/from_md/stdlib/core/result/transformations.aivi{aivi}

A useful mental model:

- use `map` for success-only changes,
- use `mapErr` when you need clearer or more structured errors,
- use `flatMap` when the next step can also fail.

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toOption` | `Result E A -> Option A` | Drops the error and keeps the success as `Some` |
| `fromOption` | `E -> Option A -> Result E A` | Turns a missing value into an explicit error |

<<< ../../snippets/from_md/stdlib/core/result/conversions.aivi{aivi}

## Combining results

| Function | Type | Description |
|----------|------|-------------|
| `flatten` | `Result E (Result E A) -> Result E A` | Removes one layer of nested `Result` |
| `orElse` | `Result E A -> Result E A -> Result E A` | Returns the first `Ok`, or a fallback result |

<<< ../../snippets/from_md/stdlib/core/result/combining.aivi{aivi}

## Relationship to other tools

- **`attempt`** turns effect failures into `Result` values.
- **`aivi.logic`** provides shared operations such as `map`, `of`, and `chain` for `Result`.
- **`aivi.option`** is useful when you only care whether a value exists, not why it is missing.
- **`do Result { ... }`** gives you readable step-by-step syntax for chaining result-producing computations.
- **`aivi.validation`** is a better fit when you want to collect several errors instead of stopping at the first one.

## Example: validation-style pipeline with `Result`

<<< ../../snippets/from_md/stdlib/core/result/pipeline_example.aivi{aivi}
