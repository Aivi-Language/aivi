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
