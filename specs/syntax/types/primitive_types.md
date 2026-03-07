# 3.1 Primitive Types

Primitive types are the foundation that other AIVI types build on.
This page separates the types the compiler must understand directly from the richer types that are best treated as library-level abstractions.

## Compiler primitives

Compiler primitives are the smallest set of types the compiler and runtime need to execute programs directly.
In v0.1, the recommended minimal set is:

<<< ../../snippets/from_md/syntax/types/primitive_types_01.aivi{aivi}

These are the types you can expect to appear everywhere: arithmetic, booleans, text processing, and basic runtime values.

## Standard library types

Everything else in this section should be treated as a standard library type, even if an implementation gives some of them a special runtime representation for performance or interop.

<<< ../../snippets/from_md/syntax/types/primitive_types_02.aivi{aivi}

In practice, this means you should think about these types in terms of their public API, not their internal storage.

## Literal forms for date and time values

Some commonly used time-related types have dedicated literal syntax:

- `2024-05-21T12:00:00Z` → `Instant`
- `~d(2024-05-21)` → `Date`
- `~t(12:00:00)` → `Time`
- `~tz(Europe/Paris)` → `TimeZone`
- `~zdt(2024-05-21T12:00:00Z[Europe/Paris])` → `ZonedDateTime`

These literals make boundary code easier to read because the intended type is visible at a glance.

## Branded nominal newtypes (`T = U!`)

A branded type creates a new type that is distinct from its underlying representation.
This is useful for IDs, validated strings, and other values that should not be mixed up accidentally.

```aivi
Email = Text!

mkEmail : Text -> Email
mkEmail = value => Email value   // explicit wrapping keeps `Email` distinct from `Text`
```

After this definition, `Email` and `Text` are different at type-check time even though they share the same underlying representation.
If you prefer `String` naming, you can define `String = Text` and then write `Email = String!`.
