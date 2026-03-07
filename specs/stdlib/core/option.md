# Standard Library: Option Utilities

<!-- quick-info: {"kind":"module","name":"aivi.option"} -->
The `aivi.option` module provides utility functions for working with `Option A` values. These complement the `??` operator and the `Functor`/`Monad` instances from `aivi.logic`.
<!-- /quick-info -->

<div class="import-badge">use aivi.option</div>

## What `Option` means

`Option A` represents “there might be a value here.”

- `Some x` means the value exists.
- `None` means it does not.

Use `Option` when absence is expected and normal, such as a missing query parameter, an empty search result, or an optional configuration value.

## Overview

This module adds small, practical helpers for checking, transforming, and converting `Option` values.

## Predicates

| Function | Type | Description |
|----------|------|-------------|
| `isSome` | `Option A -> Bool` | Returns whether the option currently holds a value |
| `isNone` | `Option A -> Bool` | Returns whether the option is empty |

<<< ../../snippets/from_md/stdlib/core/option/predicates.aivi{aivi}

## Extracting values

| Function | Type | Description |
|----------|------|-------------|
| `getOrElse` | `A -> Option A -> A` | Returns the inner value, or a default when the option is `None` |
| `getOrElseLazy` | `(Unit -> A) -> Option A -> A` | Returns the inner value, or calls a thunk to compute the default |

<<< ../../snippets/from_md/stdlib/core/option/extracting.aivi{aivi}

For simple defaults, the `??` operator is usually the shortest choice:

```aivi
name = maybeUser.name ?? "Anonymous"
```

Use `getOrElseLazy` when building the fallback value is expensive and should happen only if needed.

## Transformations

| Function | Type | Description |
|----------|------|-------------|
| `map` | `(A -> B) -> Option A -> Option B` | Changes the inner value when it exists |
| `flatMap` | `(A -> Option B) -> Option A -> Option B` | Chains two optional steps together |
| `filter` | `(A -> Bool) -> Option A -> Option A` | Keeps the value only when it satisfies the predicate |

<<< ../../snippets/from_md/stdlib/core/option/transformations.aivi{aivi}

A good rule of thumb:

- use `map` when your function always succeeds,
- use `flatMap` when your function may also return `None`,
- use `filter` when you want to reject values that do not meet a condition.

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toList` | `Option A -> List A` | Converts `None` to `[]` and `Some x` to `[x]` |
| `toResult` | `E -> Option A -> Result E A` | Turns a missing value into an explicit error |

<<< ../../snippets/from_md/stdlib/core/option/conversions.aivi{aivi}

These conversions are useful when you start with “maybe there is a value” and later need either collection-style processing or a more descriptive failure.

## Combining options

| Function | Type | Description |
|----------|------|-------------|
| `flatten` | `Option (Option A) -> Option A` | Removes one layer of nesting |
| `orElse` | `Option A -> Option A -> Option A` | Returns the first `Some`, or a fallback option |

<<< ../../snippets/from_md/stdlib/core/option/combining.aivi{aivi}

## Relationship to other tools

- **`??` operator** — best for “use this default value if the left side is missing.”
- **`aivi.logic`** — provides shared class operations such as `map`, `of`, `chain`, and `filter` where supported.
- **`aivi.result`** — use it when you need to explain *why* a value is missing.
- **`do Option { ... }`** — useful when several optional steps depend on one another.

## Example: optional pipeline

<<< ../../snippets/from_md/stdlib/core/option/pipeline_example.aivi{aivi}
