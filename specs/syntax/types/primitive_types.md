# 3.1 Primitive Types

<!-- quick-info: {"kind":"topic","name":"primitive types"} -->
Primitive types are the lowest-level value shapes that AIVI code builds on.
For broad-audience reading, it helps to separate the small set of types the compiler understands directly from the richer names you normally meet through standard-library modules and domains.
<!-- /quick-info -->

## Direct scalar carriers

These are the core scalar types you can rely on without introducing a record, ADT, or domain first:

```aivi
Unit Bool Int Float
Text Char Bytes
```

- `Unit` means “no interesting value here”.
- `Bool`, `Int`, and `Float` cover control flow and everyday arithmetic.
- `Text`, `Char`, and `Bytes` cover Unicode text, single characters, and raw binary data.

## Library-facing carrier types

The current implementation also recognizes a few carrier types because literals, runtime boundaries, or standard-library APIs refer to them directly:

```aivi
DateTime Decimal BigInt
Date TimeZone ZonedDateTime
```

Treat these as public library surface types rather than as the first stop for learning the language:

- [`aivi.chronos.instant`](../../stdlib/chronos/instant.md) explains exact UTC timestamps. Its public surface uses `Timestamp = DateTime`, so the same carrier can be treated as an instant through the `Instant` domain.
- [`aivi.chronos.calendar`](../../stdlib/chronos/calendar.md) explains `Date`, calendar arithmetic, and the `DateTime` values used at time boundaries.
- [`aivi.chronos.timezone`](../../stdlib/chronos/timezone.md) explains `TimeZone` and `ZonedDateTime`.
- [`aivi.chronos.duration`](../../stdlib/chronos/duration.md) explains elapsed-time values through the `Duration` domain over `Span`, rather than through a standalone primitive `Duration` type.

This distinction matters because names such as `Instant` and `Duration` are important in user-facing APIs, but in the current v0.1 surface they are introduced as library aliases or domains, not as raw primitive type names.

## Literal forms for time-related values

The currently documented and implementation-backed time-related literals are:

- `2024-05-21T12:00:00Z` → `DateTime`; when used with [`aivi.chronos.instant`](../../stdlib/chronos/instant.md), that same carrier is treated as a UTC `Timestamp`
- `~d(2024-05-21)` → `Date`
- `~dt(2024-05-21T12:00:00Z)` → `DateTime`
- `~tz(Europe/Paris)` → `TimeZone`
- `~zdt(2024-05-21T12:00:00[Europe/Paris])` → `ZonedDateTime`

These forms make boundary code easier to read because the intended meaning is visible at a glance.
For the general sigil syntax, see [Sigils and Literal Forms](../operators.md#sigils-and-literal-forms).
For behavior and examples, prefer the chronos module pages above.

Some older examples elsewhere in the repo still show `~t(12:00:00)`.
In the current implementation, that form infers as `DateTime`, not as a standalone `Time`, so this page intentionally points readers at the verified `~dt(...)` form instead.

Current `~tz(...)` and `~zdt(...)` behavior is exercised by `integration-tests/stdlib/aivi/chronos/timezone.aivi`.

## Branded nominal types (`T = U!`)

A branded type creates a new nominal type with the same runtime representation as an existing base type.
Use it when two values are stored the same way but should not be interchangeable at type-check time.

```aivi
Email = Text!

mkEmail : Text -> Email
mkEmail = text => Email text
```

A bare `Text` does not automatically satisfy `Email`; construct the branded value explicitly.
If you also want to hide the representation outside the defining module, combine branding with [opaque types](./opaque_types.md): `opaque Email = Text!`.
