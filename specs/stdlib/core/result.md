# Standard Library: Result Utilities

<!-- quick-info: {"kind":"module","name":"aivi.result"} -->
The `aivi.result` module provides utility functions for working with `Result E A` values. These complement the `Functor`/`Monad` instances from `aivi.logic` and the `attempt` effect operator.
<!-- /quick-info -->

<div class="import-badge">use aivi.result</div>

## Overview

`Result E A` represents a computation that may fail: either `Ok x` containing a success value, or `Err e` containing an error. This module provides common operations for inspecting, transforming, and recovering from results.

## Predicates

| Function | Type | Description |
|----------|------|-------------|
| `isOk` | `Result E A -> Bool` | Returns `True` if the result is successful |
| `isErr` | `Result E A -> Bool` | Returns `True` if the result is an error |

<<< ../../snippets/from_md/stdlib/core/result/predicates.aivi{aivi}

## Extracting Values

| Function | Type | Description |
|----------|------|-------------|
| `getOrElse` | `A -> Result E A -> A` | Unwrap or return default value |
| `getOrElseLazy` | `(E -> A) -> Result E A -> A` | Unwrap or map error to value |

<<< ../../snippets/from_md/stdlib/core/result/extracting.aivi{aivi}

## Transformations

| Function | Type | Description |
|----------|------|-------------|
| `map` | `(A -> B) -> Result E B` | Transform success value |
| `mapErr` | `(E -> F) -> Result E A -> Result F A` | Transform error value |
| `flatMap` | `(A -> Result E B) -> Result E A -> Result E B` | Chain fallible operations |

<<< ../../snippets/from_md/stdlib/core/result/transformations.aivi{aivi}

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toOption` | `Result E A -> Option A` | Discard error, keep success as `Some` |
| `fromOption` | `E -> Option A -> Result E A` | Convert `Option` to `Result` with error |

<<< ../../snippets/from_md/stdlib/core/result/conversions.aivi{aivi}

## Combining Results

| Function | Type | Description |
|----------|------|-------------|
| `flatten` | `Result E (Result E A) -> Result E A` | Remove one layer of nesting |
| `orElse` | `Result E A -> Result E A -> Result E A` | Return first `Ok`, or fallback |

<<< ../../snippets/from_md/stdlib/core/result/combining.aivi{aivi}

## Relationship to Other Modules

- **`attempt`**: Effect operator that catches errors as `Result`
- **`aivi.logic`**: Provides `Functor`, `Applicative`, `Monad` instances for `Result`
- **`aivi.option`**: Sister module for `Option A` utilities
- **`do Result { ... }`**: Monadic syntax for chaining result operations

## Example: Validation Pipeline

<<< ../../snippets/from_md/stdlib/core/result/pipeline_example.aivi{aivi}
