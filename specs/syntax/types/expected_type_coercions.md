# 3.6 Expected-Type Coercions

AIVI sometimes inserts a conversion automatically when the surrounding code already makes the destination type clear.
This keeps boundary code tidy without turning the language into a general implicit-cast system.
For the language-wide rule, see [Syntax: Operators](../operators.md#116-type-coercion-expected-type-only).

## Where coercions can happen

Expected-type coercions only happen in places where the compiler already knows the target type, such as:

- function arguments
- record fields checked against a known record type
- bindings with an explicit type annotation
- other checked positions with a known expected type, such as text interpolation, HTML child splices, or `body:` fields in HTTP request/response records

Outside those positions, no conversion is inserted.

## `ToText`

The standard library provides a `ToText` class for converting values into `Text`.
It is re-exported from the prelude, so most programs can call `toText` without an extra import.

<<< ../../snippets/from_md/syntax/types/totext_01.aivi{aivi}

::: repl
```aivi
count = 42
"There are {count} items"
// => "There are 42 items"
```
:::

Informally, when `Text` is expected and an expression has type `A`, the compiler may rewrite that expression to `toText expr` if a matching `ToText A` instance is in scope.

This is especially useful at program boundaries such as logging, headers, URLs, or templated output:

<<< ../../snippets/from_md/syntax/types/totext_02.aivi{aivi}

### Records and `ToText`

The embedded standard library also provides a broad record `ToText` instance, so record values can be rendered in expected-`Text` positions without manual wrapping.
Treat that as a convenience for structural or debug-style output.
If an API needs a stable user-facing format, prefer an explicit formatter or an opaque wrapper type with its own `ToText` instance.

## Opt-in record defaults (`ToDefault`)

When a module imports defaults from `aivi.defaults`, record literals in expected-type positions may have their missing fields filled automatically.
This is opt-in, so the behaviour is visible at the import site.

Available built-in defaults include:

- `use aivi.defaults (Option)` enables `Option _ -> None`
- `use aivi.defaults (List)` enables `List _ -> []`
- `use aivi.defaults (Bool)` enables `Bool -> False`
- `use aivi.defaults (Int, Float, Text)` enables `0`, `0.0`, and `""`

For other types, importing `ToDefault` enables instance-driven filling.
For each missing field whose expected type has a matching instance in scope, the compiler inserts a `toDefault()` call in the elaborated (fully expanded) record.

Defaults are prepended before user-written fields, so explicit fields and later spreads still win.

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_01.aivi{aivi}


## `Body` coercions

When `Body` or `ResponseBody` is expected at an HTTP boundary, the compiler inserts the following wrappers:

| Expected type | Expression type | Rewritten to |
| --- | --- | --- |
| `Body` or `ResponseBody` | Record literal `{ ... }` | `Json (toJson { ... })` |
| `Body` or `ResponseBody` | `Text` | `Plain text` |
| `Body` or `ResponseBody` | `JsonValue` | `Json jv` |
| `ResponseBody` only | `List Int` | `RawBytes bytes` |

The record-to-JSON rule only applies to a literal record expression at the coercion site.
If you already bound the payload to a name, write `Json (toJson payload)` explicitly.

That lets request-building code stay focused on the payload you mean to send:

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_02.aivi{aivi}

For the surrounding request API and `Body` type definition, see [HTTP Domain](../../stdlib/network/http.md#body). For server responses, see [HTTP Server Domain](../../stdlib/network/http_server.md#responsebody).


## `VNode` coercion

When a `VNode msg` is expected, such as in an HTML child splice, the compiler can lift text-like values into a text node automatically:

| Expression type | Rewritten to |
| --- | --- |
| `Text` | `TextNode text` |
| any `A` with `ToText A` in scope | `TextNode (toText expr)` |

If the expression already has type `VNode msg`, no wrapper is added.

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_01.aivi{aivi}


In the `{ count }` splice above, the compiler inserts `TextNode (toText count)` because the child position expects `VNode msg`.
For more detail, see [HTML sigils](../../stdlib/ui/html.md#splices) and [UI Virtual DOM](../../stdlib/ui/vdom.md).


## `Option` coercion

When `Option A` is expected and the expression does not already unify, the compiler first tries to coerce the expression to `A` using the rules above and then wraps the result in `Some`.

This composes with other coercions. For example, when `Option Body` is expected, a bare record literal becomes `Some (Json (toJson { ... }))`:

<<< ../../snippets/from_md/syntax/types/expected_type_coercions/block_03.aivi{aivi}


## Practical guideline

If a call site becomes easier to understand when written explicitly, write the conversion explicitly.
This is especially helpful for multi-step rewrites such as `Some (Json (toJson { ... }))` or when you are passing a previously bound record into `Body`.
The coercion rules are there for convenience at common boundaries, not to hide important transformations.

## Related pages

- [Syntax: Operators](../operators.md#116-type-coercion-expected-type-only) for the language-wide rule
- [HTTP Domain](../../stdlib/network/http.md#body) for `Body` and request-building examples
- [HTML sigils](../../stdlib/ui/html.md#splices) and [UI Virtual DOM](../../stdlib/ui/vdom.md) for `VNode` contexts
- [LSP Server](../../tools/lsp_server.md) if you want tooling to forbid implicit coercions
