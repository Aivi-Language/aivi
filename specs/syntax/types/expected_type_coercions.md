# 3.6 Expected-Type Coercions (Instance-Driven)

In some positions, the surrounding syntax provides an **expected type** (for example, function arguments,
record fields when a record literal is checked against a known record type, or annotated bindings).

In these expected-type positions only, the compiler may insert a conversion call when needed.
This is **not** a global implicit cast mechanism: conversions are only inserted when there is an
in-scope instance that authorizes the coercion.

## `ToText`

The standard library provides:

<<< ../../snippets/from_md/syntax/types/totext_01.aivi{aivi}

Rule (informal):

- When a `Text` is expected and an expression has type `A`, the compiler may rewrite the expression to
  `toText expr` if a `ToText A` instance is in scope.

This supports ergonomic boundary code such as HTTP requests:

<<< ../../snippets/from_md/syntax/types/totext_02.aivi{aivi}

## Record Instances

With closed structural records, `{}` denotes only the empty record.
Record-to-text coercions should therefore be provided for concrete record types (or wrappers),
rather than a single catch-all `{}` instance.

## Opt-in Record Defaults (`ToDefault`)

When a module imports markers from `aivi.defaults`, record literals in expected-type positions may
be completed with missing fields:

- `use aivi.defaults (Option)` enables `Option _ -> None`
- `use aivi.defaults (List)` enables `List _ -> []`
- `use aivi.defaults (Bool)` enables `Bool -> False`
- `use aivi.defaults (Int, Float, Text)` enables `0`, `0.0`, and `""` respectively

For other types, importing `ToDefault` enables instance-driven filling through `toDefault()` when
`ToDefault` instances are in scope.

Defaults are prepended before user-written fields, so explicit fields and later spreads still
override synthesized defaults.

## `Body` Coercions

When `Body` is expected (e.g. in an HTTP request), the compiler inserts constructor wrapping:

| Expression type | Rewritten to |
| --- | --- |
| Record literal `{ ... }` | `Json (toJson { ... })` |
| `Text` | `Plain text` |
| `JsonValue` | `Json jv` |

This enables ergonomic HTTP code:

```aivi
fetch {
  method: "POST"
  url: url
  headers: []
  body: Some { grant_type: "authorization_code", code: code }
}
```

## `Option` Coercion

When `Option A` is expected and the expression does not directly unify, the compiler attempts to
coerce the expression to type `A` using the rules above, then wraps the result in `Some`.

This chains with other coercions. For example, when `Option Body` is expected, a bare record
literal is rewritten to `Some (Json (toJson { ... }))`:

```aivi
fetch {
  method: "POST"
  url: url
  headers: []
  body: { grant_type: "authorization_code", code: code }
}
```
