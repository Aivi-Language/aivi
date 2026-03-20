# Standard Library: Result Utilities

<!-- quick-info: {"kind":"module","name":"aivi.result"} -->
The `aivi.result` module provides helper functions for working with `Result E A` values. Pair it with `aivi.logic` for shared operations such as `map`, `chain`, `bimap`, and `alt`, and with `attempt` when you want to capture effect failures as data.
<!-- /quick-info -->

<div class="import-badge">use aivi.result</div>

Add `use aivi.logic` as well when you want the shared `Result` operations such as `map`, `chain`, `bimap`, or `alt`.

## What `Result` means

`Result E A` represents a computation that can succeed or fail.

- `Ok x` holds a successful value.
- `Err e` holds an error value.

Use `Result` when failure is expected and you want to keep it explicit instead of throwing exceptions or hiding error cases.

Mental model: `Result` answers “did this step work, and if not, why not?”.

## Start here

Reach for `Result` when each step may fail and later work depends on the earlier successful value.
That is the usual shape for parsing, loading configuration, checking permissions, opening files, and any other workflow where “stop here and return the error” is the right behavior.

If a missing value is normal, use [`Option`](option.md) instead. If you need to report several independent problems together, use [`Validation`](validation.md).

## Choosing between `Option`, `Result`, and `Validation`

| If the situation is... | Use | Why |
| --- | --- | --- |
| the value may simply be absent | [`Option`](option.md) | absence is normal and needs no extra explanation |
| the step can fail with one explicit reason | `Result` | `Err e` keeps the reason and short-circuits |
| many independent checks should report all problems | [`Validation`](validation.md) | failures accumulate instead of stopping at the first one |

## Overview

This page covers the module-specific helpers in `aivi.result`, plus the shared [`aivi.logic`](logic.md) operations that are most often used with `Result`.

AIVI uses the shared name `chain` for “run the next fallible step if this one succeeded” rather than a separate `flatMap` helper.

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

::: repl
```aivi
Ok 42 |> isOk
// => True
Err "fail" |> getOrElse 0
// => 0
Ok 10 |> map (_ * 2)
// => Ok 20
```
:::

`getOrElseLazy` is handy when the fallback depends on the error details.

## Transformations

`map` and `chain` come from [`aivi.logic`](logic.md). `mapErr` is the `aivi.result` helper for changing only the error side.

| Function | Source | Type | Description |
|----------|--------|------|-------------|
| `map` | [`aivi.logic`](logic.md) | `(A -> B) -> Result E A -> Result E B` | Transforms the success value while leaving errors alone |
| `mapErr` | `aivi.result` | `(E -> F) -> Result E A -> Result F A` | Transforms the error value while leaving successes alone |
| `chain` | [`aivi.logic`](logic.md) | `(A -> Result E B) -> Result E A -> Result E B` | Chains fallible operations together |

<<< ../../snippets/from_md/stdlib/core/result/block_01.aivi{aivi}


A useful mental model:

- use `map` for success-only changes,
- use `mapErr` when you need clearer or more structured errors,
- use `chain` when the next step can also fail.

### A readable fallible pipeline

Breaking a `Result` workflow into named steps usually reads better than deeply nested calls:

<<< ../../snippets/from_md/stdlib/core/result/block_02.aivi{aivi}


Each binding answers one question: did the previous step succeed, and if so, what is the next fallible step?

### A small `mapErr` example

`mapErr` is especially useful when the low-level error is technically correct but still needs domain context:

<<< ../../snippets/from_md/stdlib/core/result/block_03.aivi{aivi}


That keeps the original failure information while making the error more helpful at the call site.

## Conversions

| Function | Type | Description |
|----------|------|-------------|
| `toOption` | `Result E A -> Option A` | Drops the error and keeps the success as `Some` |
| `fromOption` | `E -> Option A -> Result E A` | Turns a missing value into an explicit error |

<<< ../../snippets/from_md/stdlib/core/result/conversions.aivi{aivi}

## Combining results

When you need fallback choice or want to collapse one nested `Result`, use the shared operations from [`aivi.logic`](logic.md).

| Operation | Source | Type | Description |
|-----------|--------|------|-------------|
| `chain (inner => inner)` | [`aivi.logic`](logic.md) | `Result E (Result E A) -> Result E A` | Removes one layer of nesting when both layers use the same error type |
| `alt` | [`aivi.logic`](logic.md) | `Result E A -> Result E A -> Result E A` | Returns the first `Ok`, or a fallback result |

<<< ../../snippets/from_md/stdlib/core/result/block_04.aivi{aivi}


## Relationship to other tools

- **[`attempt`](../../syntax/effects.md)** turns effect failures into `Result` values.
- **[`aivi.logic`](logic.md)** provides shared `Result` operations such as `map`, `chain`, `bimap`, `alt`, and `of`.
- **[`aivi.option`](option.md)** is useful when you only care whether a value exists, not why it is missing.
- **[Flow Syntax](../../syntax/flows.md)** is useful when several result-producing steps read better left-to-right.
- **[`aivi.validation`](validation.md)** is a better fit when you want to collect several errors instead of stopping at the first one.

## Example: validation-style pipeline with `Result`

This example parses raw input, adds domain context to parse failures, then continues with later checks only when the earlier steps succeeded:

<<< ../../snippets/from_md/stdlib/core/result/block_05.aivi{aivi}


Here `parseJson` can fail first, `validateSchema` and `checkPermissions` depend on the parsed value, and `getOrElse` turns the final `Result` into a plain value at the boundary where a fallback makes sense.
