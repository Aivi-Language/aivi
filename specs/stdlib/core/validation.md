# Standard Library: Validation

<!-- quick-info: {"kind":"module","name":"aivi.validation"} -->
The `aivi.validation` module provides the `Validation E A` type, which is used for data validation that accumulates multiple errors, unlike `Result` which short-circuits on the first error.
<!-- /quick-info -->

<div class="import-badge">use aivi.validation</div>

<<< ../../snippets/from_md/stdlib/core/validation/standard_library_validation.aivi{aivi}

## Start here

Reach for `Validation` when you want to tell a user **everything** that is wrong with some input in one pass.
Typical examples are form fields, config files, imported CSV rows, or any other case where independent checks can all run before you decide whether the whole value is acceptable.

## What `Validation` is for

Use `Validation E A` when you want to check several inputs and report **all** of the problems you found, not just the first one.

That makes it a good fit for tasks such as:

- validating forms,
- checking configuration files,
- decoding structured input,
- verifying data imported from external systems.

If you want the computation to stop at the first failure, use `Result` instead.

## Quick chooser

| If the situation is... | Use | Why |
| --- | --- | --- |
| the value might be missing, and that is normal | [`Option`](option.md) | there is no extra error story to tell |
| one step can fail and the next step depends on it | [`Result`](result.md) | stop early and keep one explicit success-or-error path |
| several checks are independent and you want all problems | `Validation` | accumulate errors instead of stopping at the first one |

## 1. The `Validation` type

`Validation E A` looks a lot like `Result E A`, but it is designed for a different workflow.

- `Result` is best for sequential steps where each step depends on the previous one.
- `Validation` is best for independent checks that can all be run and then combined.

`Validation` is an [`Applicative`](logic.md#applicative) rather than a [`Monad`](logic.md#monad). In plain language, that means AIVI can combine independent validations side by side and accumulate their errors, but it is not the tool for `chain`-style workflows where each later check depends on an earlier successful value.

If those words are unfamiliar, the practical takeaway is simple: `Validation` is for checks that can run independently, such as “name is present” and “email looks valid”, then report all failures together.

Another way to say it:

- `Result` is for “check step 1, then decide step 2”
- `Validation` is for “run these checks separately, then merge what they found”

<<< ../../snippets/from_md/stdlib/core/validation/the_validation_type.aivi{aivi}

::: repl
```aivi
ok = Valid 42
bad = Invalid ["too short", "missing @"]
ok |> map (_ + 1)
// => Valid 43
```
:::

## 2. Combining validations with `ap`

In everyday AIVI code, the main combination helper is `ap` (the `Applicative` combination operator from [`aivi.logic`](logic.md)):

<<< ../../snippets/from_md/stdlib/core/validation/block_01.aivi{aivi}


That signature is why the most common shape is `Validation (List E) A`: each failed check contributes a list of errors, and `ap` appends the lists when more than one check fails.
If you want the typeclass background, this is the practical `Validation` version of [`Applicative`](logic.md#applicative) combination, and the list of errors is the [`Semigroup`](logic.md#semigroup) that makes accumulation possible.

<<< ../../snippets/from_md/stdlib/core/validation/block_02.aivi{aivi}


Here, `result` is `Invalid ["Name is required", "Age must be non-negative"]`.

## 3. Creating validations

Helper functions make it easy to lift either a valid value or one or more errors into the `Validation` type. A good pattern is to validate each field separately, then combine those field checks at the end.
When you plan to combine checks with `ap`, wrap each single failure in a one-element list such as `Invalid ["Email is required"]` so later checks can append their own errors.

<<< ../../snippets/from_md/stdlib/core/validation/creating_validations.aivi{aivi}

## 4. Converting from and to `Result`

Sometimes a single field check is naturally written as a `Result`, and then lifted into a larger validation pipeline that should accumulate errors across fields.
This is a common pattern when each individual parser or decoder already returns `Result`, but the whole form or configuration load should report multiple problems at once.
`fromResult` turns `Err e` into `Invalid [e]`, while `toResult` preserves the entire accumulated error value when you want to leave the validation-oriented part of the pipeline.

<<< ../../snippets/from_md/stdlib/core/validation/converting_from_to_result.aivi{aivi}

## 5. `DecodeError` ADT

For validations around standard data sources such as JSON, environment variables, or databases, the standard library uses `DecodeError` to capture both **where** the problem occurred and **what** went wrong.

<<< ../../snippets/from_md/stdlib/core/validation/block_03.aivi{aivi}


- `path` is the location inside the incoming value, such as `["user", "preferences", "theme"]`.
- `message` explains the actual mismatch, such as `Expected Text, got Int`.

`formatDecodeError` turns that structured value into a user-facing message. For example, a `DecodeError` with path `["user", "preferences", "theme"]` renders as `at $.user.preferences.theme: Expected Text, got Int`.
An empty path renders as `at $.: ...`, which is how root-level decode failures are displayed in the current implementation and tests.

This is part of the foundation for AIVI's type-safe bindings at source boundaries. See also [`aivi.json`](../data/json.md) for a larger decoding workflow built on `Validation` and `DecodeError`.
