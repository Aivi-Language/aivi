# 3.1 Primitive Types

AIVI distinguishes:

- **Compiler primitives**: types the compiler/runtime must know about to execute code.
- **Standard library types**: types defined in AIVI source (possibly with compiler-known representation in early implementations).

In v0.1, the recommended minimal set of **compiler primitives** is:

<<< ../../snippets/from_md/syntax/types/primitive_types_01.aivi{aivi}

Everything else below should be treated as a **standard library type** (even if an implementation chooses to represent it specially at first for performance/interop).

<<< ../../snippets/from_md/syntax/types/primitive_types_02.aivi{aivi}

Numeric suffixes:

* `2024-05-21T12:00:00Z` → `Instant`
* `~d(2024-05-21)` → `Date`
* `~t(12:00:00)` → `Time`
* `~tz(Europe/Paris)` → `TimeZone`
* `~zdt(2024-05-21T12:00:00Z[Europe/Paris])` → `ZonedDateTime`

## Branded nominal newtypes (`T = U!`)

You can define a distinct nominal type from an existing base type with `!`:

```aivi
Email = Text!

mkEmail : Text -> Email
mkEmail = value => Email value
```

`Email` is now distinct from `Text` at type-check time, while still using the `Email` constructor to wrap values explicitly.
If you prefer `String` naming, define `String = Text` and then `Email = String!`.
