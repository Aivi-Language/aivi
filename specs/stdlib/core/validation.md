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

`Validation` is an **Applicative** rather than a **Monad**. In plain language, that means AIVI can combine independent validations side by side and accumulate their errors, but it does not support `chain` for workflows where each later check depends on an earlier successful value.

If those words are unfamiliar, the practical takeaway is simple: `Validation` is for checks that can run independently, such as “name is present” and “email looks valid”, then report all failures together.

Another way to say it:

- `Result` is for “check step 1, then decide step 2”
- `Validation` is for “run these checks separately, then merge what they found”

<<< ../../snippets/from_md/stdlib/core/validation/the_validation_type.aivi{aivi}

## 2. Applicative instance

When two `Validation` values are combined applicatively and both are invalid, their errors are concatenated. That requires `E` to be a `Semigroup`, which simply means the error type knows how to merge two error values into one. A list of errors is the most common choice.

For everyday use, you can read “`E` must be a `Semigroup`” as “the error type must know how to join two failures into one combined failure.”

<<< ../../snippets/from_md/stdlib/core/validation/applicative_instance.aivi{aivi}

## 3. Creating validations

Helper functions make it easy to lift either a valid value or one or more errors into the `Validation` type. A good pattern is to validate each field separately, then combine those field checks at the end.

<<< ../../snippets/from_md/stdlib/core/validation/creating_validations.aivi{aivi}

## 4. Converting from and to `Result`

Sometimes a single field check is naturally written as a `Result`, and then lifted into a larger validation pipeline that should accumulate errors across fields.
This is a common pattern when each individual parser or decoder already returns `Result`, but the whole form or configuration load should report multiple problems at once.

<<< ../../snippets/from_md/stdlib/core/validation/converting_from_to_result.aivi{aivi}

## 5. `DecodeError` ADT

For validations around standard data sources such as JSON, environment variables, or databases, the standard library uses `DecodeError` to capture both **where** the problem occurred and **what** went wrong.

<<< ../../snippets/from_md/stdlib/core/validation/decodeerror_adt.aivi{aivi}

This is part of the foundation for AIVI's type-safe bindings at `Source` boundaries.
