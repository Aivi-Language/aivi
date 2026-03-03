# Standard Library: Option Utilities

<!-- quick-info: {"kind":"module","name":"aivi.option"} -->
The `aivi.option` module provides utility functions for working with `Option A` values. These complement the `??` operator and the `Functor`/`Monad` instances from `aivi.logic`.
<!-- /quick-info -->

<div class="import-badge">use aivi.option</div>

## Overview

`Option A` represents an optional value: either `Some x` containing a value, or `None` representing absence. This module provides common operations for inspecting, transforming, and extracting values from options.

## Predicates

| Function | Type | Description |
|----------|------|-------------|
| `isSome` | `Option A -> Bool` | Returns `True` if the option contains a value |
| `isNone` | `Option A -> Bool` | Returns `True` if the option is empty |

<<< ../../snippets/from_md/stdlib/core/option/predicates.aivi{aivi}

## Extracting Values

| Function | Type | Description |
|----------|------|-------------|
| `getOrElse` | `A -> Option A -> A` | Unwrap or return default value |
| `getOrElseLazy` | `(Unit -> A) -> Option A -> A` | Unwrap or call thunk for default |

<<< ../../snippets/from_md/stdlib/core/option/extracting.aivi{aivi}

For simple cases, prefer the `??` operator:

```aivi
name = maybeUser.name ?? "Anonymous"
```

## Transformations

| Function | Type | Description |
|----------|------|-------------|
| `map` | `(A -> B) -> Option A -> Option B` | Transform inner value if present |
| `flatMap` | `(A -> Option B) -> Option A -> Option B` | Chain operations that may fail |
| `filter` | `(A -> Bool) -> Option A -> Option A` | Keep value only if predicate holds |

<<< ../../snippets/from_md/stdlib/core/option/transformations.aivi{aivi}

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toList` | `Option A -> List A` | Empty list for `None`, singleton for `Some` |
| `toResult` | `E -> Option A -> Result E A` | Convert to `Result` with error value |

<<< ../../snippets/from_md/stdlib/core/option/conversions.aivi{aivi}

## Combining Options

| Function | Type | Description |
|----------|------|-------------|
| `flatten` | `Option (Option A) -> Option A` | Remove one layer of nesting |
| `orElse` | `Option A -> Option A -> Option A` | Return first `Some`, or fallback |

<<< ../../snippets/from_md/stdlib/core/option/combining.aivi{aivi}

## Relationship to Other Modules

- **`??` operator**: Built-in coalescing for simple default values
- **`aivi.logic`**: Provides `Functor`, `Applicative`, `Monad` instances for `Option`
- **`aivi.result`**: Sister module for `Result E A` utilities
- **`do Option { ... }`**: Monadic syntax for chaining option operations

## Example: Pipeline

<<< ../../snippets/from_md/stdlib/core/option/pipeline_example.aivi{aivi}
