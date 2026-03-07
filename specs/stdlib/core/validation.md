# Standard Library: Validation

<!-- quick-info: {"kind":"module","name":"aivi.validation"} -->
The `aivi.validation` module provides the `Validation E A` type, which is used for data validation that accumulates multiple errors, unlike `Result` which short-circuits on the first error.
<!-- /quick-info -->

<div class="import-badge">use aivi.validation</div>

<<< ../../snippets/from_md/stdlib/core/validation/standard_library_validation.aivi{aivi}

## What `Validation` is for

Use `Validation E A` when you want to check several inputs and report **all** of the problems you found, not just the first one.

That makes it a good fit for tasks such as:

- validating forms,
- checking configuration files,
- decoding structured input,
- verifying data imported from external systems.

If you want the computation to stop at the first failure, use `Result` instead.

## 1. The `Validation` type

`Validation E A` looks a lot like `Result E A`, but it is designed for a different workflow.

- `Result` is best for sequential steps where each step depends on the previous one.
- `Validation` is best for independent checks that can all be run and then combined.

`Validation` is an **Applicative** rather than a **Monad**. In plain language, that means AIVI can combine independent validations and accumulate their errors, but it does not support `chain` for dependent step-by-step validation.

<<< ../../snippets/from_md/stdlib/core/validation/the_validation_type.aivi{aivi}

## 2. Applicative instance

When two `Validation` values are combined applicatively and both are invalid, their errors are concatenated. That requires `E` to be a `Semigroup`; a list of errors is the most common choice.

<<< ../../snippets/from_md/stdlib/core/validation/applicative_instance.aivi{aivi}

## 3. Creating validations

Helper functions make it easy to lift either a valid value or one or more errors into the `Validation` type.

<<< ../../snippets/from_md/stdlib/core/validation/creating_validations.aivi{aivi}

## 4. Converting from and to `Result`

Sometimes a single field check is naturally written as a `Result`, and then lifted into a larger validation pipeline that should accumulate errors across fields.

<<< ../../snippets/from_md/stdlib/core/validation/converting_from_to_result.aivi{aivi}

## 5. `DecodeError` ADT

For validations around standard data sources such as JSON, environment variables, or databases, the standard library uses `DecodeError` to capture both **where** the problem occurred and **what** went wrong.

<<< ../../snippets/from_md/stdlib/core/validation/decodeerror_adt.aivi{aivi}

This is part of the foundation for AIVI's type-safe bindings at `Source` boundaries.
