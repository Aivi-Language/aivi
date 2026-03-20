# Standard Library: Option Utilities

<!-- quick-info: {"kind":"module","name":"aivi.option"} -->
The `aivi.option` module provides utility functions for working with `Option A` values. This page also points out the shared [`aivi.logic`](logic.md) operations that `Option` supports, such as `map`, `chain`, `filter`, and `alt`.
<!-- /quick-info -->

<div class="import-badge">use aivi.option</div>

Add `use aivi.logic` as well when you want the shared `Option` operations such as `map`, `chain`, `filter`, or `alt`.

## What `Option` means

`Option A` represents “there might be a value here.”

- `Some x` means the value exists.
- `None` means it does not.

Use `Option` when absence is expected and normal, such as a missing query parameter, an empty search result, or an optional configuration value.

Mental model: `Option` answers only one question—“is there a value?”—and intentionally says nothing about *why* it is missing.

## Start here

Use `Option` when:

- a missing value is normal,
- you do not need to explain the reason,
- you are not collecting several independent failures.

Do **not** use `Option` when the caller needs a real error message or error value. In those cases, move to [`Result`](result.md) or [`Validation`](validation.md).

For example, a missing `page` query parameter is a good `Option Int`; a broken config file path is usually a `Result ConfigError Path`.

## Choosing between `Option`, `Result`, and `Validation`

| If the situation is... | Use | Why |
| --- | --- | --- |
| the only question is “is there a value?” | `Option` | `Some` or `None` tells the whole story |
| failure needs a reason such as `NotFound` or `BadPort` | [`Result`](result.md) | `Err e` preserves the reason |
| several independent checks should all report problems | [`Validation`](validation.md) | failures accumulate instead of short-circuiting |

## Overview

This page covers two kinds of tools:

- `aivi.option` helpers for checking, extracting, and converting `Option` values,
- shared [`aivi.logic`](logic.md) operations that already work with `Option`.

## Predicates

These helpers are most useful near the edges of your program when you need a direct branch. In pipelines, `map`, `chain`, and `??` often read more clearly.

| Function | Type | Description |
|----------|------|-------------|
| `isSome` | `Option A -> Bool` | Returns whether the option currently holds a value |
| `isNone` | `Option A -> Bool` | Returns whether the option is empty |

<<< ../../snippets/from_md/stdlib/core/option/predicates.aivi{aivi}

::: repl
```aivi
isSome (Some 42)
// => True
isNone None
// => True
Some 5 |> map (_ + 1)
// => Some 6
None |> getOrElse 0
// => 0
```
:::

## Extracting values

| Function | Type | Description |
|----------|------|-------------|
| `getOrElse` | `A -> Option A -> A` | Returns the inner value, or a default when the option is `None` |
| `getOrElseLazy` | `(Unit -> A) -> Option A -> A` | Returns the inner value, or calls a thunk to compute the default |

<<< ../../snippets/from_md/stdlib/core/option/extracting.aivi{aivi}

For simple defaults, the `??` operator is usually the shortest choice:

<<< ../../snippets/from_md/stdlib/core/option/block_01.aivi{aivi}


Use `getOrElseLazy` when building the fallback value is expensive and should happen only if needed.

## Transformations

`Option` participates in the shared classes from [`aivi.logic`](logic.md). In AIVI, the shared name is `chain` rather than a separate `flatMap` helper.

| Function | Type | Description |
|----------|------|-------------|
| `map` | `(A -> B) -> Option A -> Option B` | Changes the inner value when it exists |
| `chain` | `(A -> Option B) -> Option A -> Option B` | Chains two optional steps together |
| `filter` | `(A -> Bool) -> Option A -> Option A` | Keeps the value only when it satisfies the predicate |

<<< ../../snippets/from_md/stdlib/core/option/block_02.aivi{aivi}


A good rule of thumb:

- use `map` when your function always succeeds,
- use `chain` when your function may also return `None`,
- use `filter` when you want to reject values that do not meet a condition.

### A readable optional pipeline

When an optional workflow starts to feel nested, split it into named steps:

<<< ../../snippets/from_md/stdlib/core/option/block_03.aivi{aivi}


This reads better than packing all of that into one expression, and it makes the “where can this become `None`?” points obvious.

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toList` | `Option A -> List A` | Converts `None` to `[]` and `Some x` to `[x]` |
| `toResult` | `E -> Option A -> Result E A` | Turns a missing value into an explicit error |

<<< ../../snippets/from_md/stdlib/core/option/conversions.aivi{aivi}

These conversions are useful when you start with “maybe there is a value” and later need either collection-style processing or a more descriptive failure.

## Combining options

For fallback choice and one-layer flattening, reach for the shared [`aivi.logic`](logic.md) operations that `Option` already implements.

<<< ../../snippets/from_md/stdlib/core/option/block_04.aivi{aivi}


`chain (inner => inner)` is the standard one-layer flattening pattern. `alt` is the shared fallback-choice operation that many other libraries call `orElse`.

## Relationship to other tools

- **`??` operator** — best for “use this default value if the left side is missing.”
- **[`aivi.logic`](logic.md)** — provides shared class operations such as `map`, `of`, `chain`, `filter`, and `alt` where supported.
- **[`aivi.result`](result.md)** — use it when you need to explain *why* a value is missing or invalid.
- **[Flow Syntax](../../syntax/flows.md)** — useful when several optional steps read better as one flat left-to-right flow.

## Example: lookup, validate, and default an optional email

Assume each user record stores `email : Option Text`. This example looks up a user, keeps only email-like strings, normalizes them, and falls back to a default address.

<<< ../../snippets/from_md/stdlib/core/option/block_05.aivi{aivi}

