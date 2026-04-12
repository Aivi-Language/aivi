# Data shapes: records, constructors, and domains

AIVI currently uses three distinct surface models for "structured data". They overlap in day-to-day use,
but they are not interchangeable.

## Records

Records are structural and named:

```aivi
type User = {
    id: Int,
    name: Text
}
```

- Constructed with record literals, not constructor application.
- Projected by field name (`user.name`).
- Closed by default and support record-row transforms such as `Pick`, `Omit`, and `Rename`.

## Constructor-backed `type` declarations

Closed sums and single-constructor positional products still live under `type`:

```aivi
type Option A = None | Some A

type Date =
  Date year:Year month:Month day:Day
```

- Constructors are ordinary curried values.
- Single-constructor multi-field declarations are still positional product constructors.
- Field labels on constructor payloads are documentation/diagnostic metadata; construction stays positional.
- Pattern matching uses the constructor name, so these remain nominal closed ADTs rather than records.

This means product constructors are still a first-class part of the language, not legacy syntax.

## Domains

Domains are nominal wrappers over one declared carrier type:

```aivi
domain Duration over Int = {
    type millis : Int -> Duration
    millis = raw => raw
}
```

- Construction and unwrapping are explicit.
- There are no implicit casts to or from the carrier.
- Domain bodies can define suffixes, operators, and named members.
- `.carrier` is synthesized for every domain.

Domains are best for "this is really an `Int`/`Text`/`List A`, but with a distinct meaning and its own
operations".

## `aivi.date` today

`aivi.date` currently splits its surface intentionally:

- `Date`, `TimeOfDay`, `DateTime`, and `ZonedDateTime` are constructor-backed product types.
- `DateDelta` is the nominal domain.

So the manual entry

```aivi
type Date =
  Date year:Year month:Month day:Day
```

matches the current stdlib source. `Date` has not been migrated to a domain.

## Practical rule of thumb

- Use a **record** when named structural fields and field projection are the main affordance.
- Use a **constructor-backed type** when constructor identity and pattern matching matter, or when a
  compact positional product is a good fit.
- Use a **domain** when you want a distinct nominal wrapper over exactly one carrier type with
  domain-specific members or literal suffixes.

## Sources

- `syntax.md`
- `manual/guide/types.md`
- `manual/guide/domains.md`
- `manual/stdlib/date.md`
- `stdlib/aivi/date.aivi`
- `stdlib/aivi/pair.aivi`
