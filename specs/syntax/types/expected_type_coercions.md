# 3.6 Expected-Type Coercions

AIVI sometimes inserts a conversion automatically when the surrounding code already makes the destination type clear.
This keeps boundary code tidy without turning the language into a general implicit-cast system.

## Where coercions can happen

Expected-type coercions only happen in places where the compiler already knows the target type, such as:

- function arguments
- record fields checked against a known record type
- bindings with an explicit type annotation

Outside those positions, no conversion is inserted.

## `ToText`

The standard library provides a `ToText` class for converting values into `Text`.

<<< ../../snippets/from_md/syntax/types/totext_01.aivi{aivi}

Informally, when `Text` is expected and an expression has type `A`, the compiler may rewrite that expression to `toText expr` if a matching `ToText A` instance is in scope.

This is especially useful at program boundaries such as logging, headers, URLs, or templated output:

<<< ../../snippets/from_md/syntax/types/totext_02.aivi{aivi}

## Record instances

With closed structural records, `{}` means only the empty record.
That is why record-to-text coercions should be defined for concrete record shapes or wrapper types, not through one catch-all record instance.

## Opt-in record defaults (`ToDefault`)

When a module imports defaults from `aivi.defaults`, record literals in expected-type positions may be completed with missing fields.
This is opt-in, so the behaviour is visible at the import site.

Available built-in defaults include:

- `use aivi.defaults (Option)` enables `Option _ -> None`
- `use aivi.defaults (List)` enables `List _ -> []`
- `use aivi.defaults (Bool)` enables `Bool -> False`
- `use aivi.defaults (Int, Float, Text)` enables `0`, `0.0`, and `""`

For other types, importing `ToDefault` enables instance-driven filling through `toDefault()` when matching instances are in scope.

Defaults are prepended before user-written fields, so explicit fields and later spreads still win.

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_01.aivi{aivi}


## `Body` coercions

When `Body` is expected, such as in an HTTP request, the compiler inserts the following wrappers:

| Expression type | Rewritten to |
| --- | --- |
| Record literal `{ ... }` | `Json (toJson { ... })` |
| `Text` | `Plain text` |
| `JsonValue` | `Json jv` |

That lets request-building code stay focused on the payload you mean to send:

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_02.aivi{aivi}


## `Option` coercion

When `Option A` is expected and the expression does not already unify, the compiler first tries to coerce the expression to `A` using the rules above and then wraps the result in `Some`.

This composes with other coercions. For example, when `Option Body` is expected, a bare record literal becomes `Some (Json (toJson { ... }))`:

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_03.aivi{aivi}


## Practical guideline

If a call site becomes easier to understand when written explicitly, write the conversion explicitly.
The coercion rules are there for convenience at common boundaries, not to hide important transformations.
