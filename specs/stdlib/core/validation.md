# Standard Library: Validation

<!-- quick-info: {"kind":"module","name":"aivi.validation"} -->
The `aivi.validation` module provides the `Validation E A` type, which is used for data validation that accumulates multiple errors, unlike `Result` which short-circuits on the first error.
<!-- /quick-info -->

<div class="import-badge">use aivi.validation</div>

<<< ../../snippets/from_md/stdlib/core/validation/standard_library_validation.aivi{aivi}

## 1. The Validation Type

`Validation E A` behaves like `Result`, but it is an **Applicative** rather than a **Monad** (it does not provide `chain`). When using `ap` (or applicative combinations) over multiple `Validation` values, errors from both sides are concatenated (assuming `E` is a `Semigroup`).

<<< ../../snippets/from_md/stdlib/core/validation/the_validation_type.aivi{aivi}

## 2. Applicative Instance

Because `Validation` expects `E` to be a `Semigroup` (usually a `List`), combining two `Invalid` values concatenates their contents.

<<< ../../snippets/from_md/stdlib/core/validation/applicative_instance.aivi{aivi}

## 3. Creating Validations

Helper functions make it easy to lift values or errors into the `Validation` applicative.

<<< ../../snippets/from_md/stdlib/core/validation/creating_validations.aivi{aivi}

## 4. Converting from/to Result

Sometimes you want to validate a single field (returning a `Result`) and lift it into an accumulated `Validation` pipeline.

<<< ../../snippets/from_md/stdlib/core/validation/converting_from_to_result.aivi{aivi}

## 5. DecodeError ADT

For standard data source validations (JSON, Env, DB), the standard library uses `DecodeError` to track precisely *where* a validation failed and *why*.

<<< ../../snippets/from_md/stdlib/core/validation/decodeerror_adt.aivi{aivi}

This is the foundation for AIVI's automatic typesafe bindings at `Source` boundaries.
