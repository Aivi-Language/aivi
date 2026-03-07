# Domains

Domains let AIVI give familiar syntax a domain-specific meaning. Instead of teaching the core language every rule about calendars, units, geometry, or colors, a domain says how operators and suffix literals should behave for a particular kind of value.

## What a domain is

A domain is a bundle of semantics for a **carrier type** such as a date, duration, vector, or color. A domain may provide:

- operator meanings such as `+`, `-`, `*`, or `×`
- typed suffix literals such as `10ms`, `2h`, or `30deg`
- helper types for changes or measurements, often called **deltas**
- compile-time validation for domain-owned syntax

That means code can stay concise without becoming untyped. For example, `30d` is not a string; it is a typed value supplied by an active domain.

## What domains are for

Domains are useful when plain numbers are not enough. Reach for them when you want:

- **typed units** instead of “magic numbers”
- **readable operators** for a specific problem space
- **literal suffixes** that compile into real values
- **clear separation** between the core language and specialized rules

Typical examples include time and calendar arithmetic, geometry, matrices, angles, and UI color adjustment.

## Using an existing domain

To use a domain, import it with `use`. This brings its operator functions and literal templates into scope.

<<< ../snippets/from_md/syntax/domains/using_domains.aivi{aivi}

```aivi
use aivi.chronos.calendar (domain Calendar)

dueDate = issuedOn + 30d   -- Calendar provides both `+` and the `d` suffix here
```

### Importing a domain explicitly

Domains are imported separately from ordinary values and types.

- Export a domain with `export domain Calendar`
- Import it with `use aivi.chronos.calendar (domain Calendar)`

Importing the domain activates its members, including operator definitions like `(+)` and literal templates like `1d`.

> Domain names are not ordinary runtime values. You do not pass `Calendar` around as a term-level value; you import it so its syntax becomes available in the current module.

## Typed deltas and suffix literals

Domains often introduce **delta** values: typed changes such as “three days”, “five meters”, or “twenty degrees”. These are commonly written as numeric literals with a suffix.

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_01.aivi{aivi}

These literals are fully typed. Depending on the active domain, `10m` might mean a duration, a distance, or something else entirely.

Common standard-library suffixes include:

| Suffix | Domain | Type | Module |
| --- | --- | --- | --- |
| `10ms`, `1s`, `5min`, `2h` | Duration | `Duration` | [aivi.chronos.duration](../stdlib/chronos/duration.md) |
| `1d`, `2w`, `3mo`, `1y` | Calendar | `CalendarDelta` | [aivi.chronos.calendar](../stdlib/chronos/calendar.md) |
| `20deg`, `1.2rad` | Angle | `Angle` | [aivi.math](../stdlib/math/math.md) |
| `10l`, `5s`, `30h` | Color | `ColorDelta` | [aivi.color](../stdlib/ui/color.md) |

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_02.aivi{aivi}

## Applying suffixes to computed values

You can also attach a suffix to a parenthesized expression. This is the form to use when the numeric part is stored in a variable or computed first.

<<< ../snippets/from_md/syntax/domains/suffix_application_variables.aivi{aivi}

Write the suffix immediately after the closing `)`:

- correct: `(x)kg`
- incorrect: `(x) kg`

## Domain-owned operators

Domains may define operator behavior that goes beyond plain integer arithmetic. That includes familiar operators such as `+` and `*`, and also `×` for transform-style multiplication.

### Overloading the same operator inside one domain

A single domain body may contain multiple entries for the same operator token as long as the full function types are different. In practice, the compiler resolves the operator by looking at the carrier type on the left-hand side and the inferred type on the right-hand side.

<<< ../snippets/from_md/syntax/domains/within_domain_operator_overloads_rhs_typed.aivi{aivi}

A good convention is:

- use `×` for structural products such as matrix-matrix or matrix-vector multiplication
- use `*` for scalar scaling

Rules:

1. A domain declaration still has exactly one carrier type: `domain D over Carrier`
2. Reusing the same operator token is allowed only when the full `LHS -> RHS -> Result` types are pairwise distinct
3. Resolution succeeds only when exactly one overload matches the inferred operand types

## Supported operator hooks in v0.1

In v0.1, the surface language has built-in syntax for operators, and domains may supply the meaning for part of that syntax.

| Operator | Built-in meaning | Domain meaning (when a non-`Int` carrier is involved) |
| --- | --- | --- |
| `+`, `-`, `*`, `×`, `/`, `%` | `Int` arithmetic | Resolved to an in-scope operator function such as `(+)` |
| `<`, `<=`, `>`, `>=` | `Int` ordering | Resolved to an in-scope operator function such as `(<)` returning `Bool` |

These operators are **not** domain-resolved in v0.1 and always keep their built-in meaning: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`.

See also: [Operators and Context](operators.md#114-domains-and-operator-meaning).

## Literal templates and name collisions

Suffix literals are implemented through template functions named `1{suffix}`:

- `10ms` uses `1ms`
- `(x)ms` also uses `1ms`

Domains usually define these templates in the domain body.

If two imported domains define the same template name, the compiler does not currently disambiguate them by carrier type. In practice, avoid collisions by:

- importing only one conflicting domain in a module
- using selective imports or hiding
- calling an explicit constructor or helper function instead of using the suffix literal

## Defining your own domain

Define a domain when you want problem-specific syntax that stays type-safe and predictable.

### Syntax

<<< ../snippets/from_md/syntax/domains/syntax.aivi{aivi}

### Example: a simple color domain

<<< ../snippets/from_md/syntax/domains/example_a_simple_color_domain.aivi{aivi}

### How the compiler reads a domain expression

When you write a domain-owned expression, the compiler starts from the carrier type and then resolves the operator and any suffix literals through the domain currently in scope.

<<< ../snippets/from_md/syntax/domains/interpretation.aivi{aivi}

For a value like `red + 10l`, the flow is:

1. infer that `red` has the carrier type, for example `Rgb`
2. find an in-scope domain declared over that carrier, for example `Color`
3. desugar `10l` using the domain’s template for that suffix
4. resolve `+` to the domain’s `(+)` definition

This only works when the domain itself is in scope, for example `use aivi.color (domain Color)`. Importing only the carrier type is not enough.

## Multi-carrier domains

Sometimes one idea applies to several carrier types, such as both `Vec2` and `Vec3`. In v0.1, write one domain declaration per carrier type.

<<< ../snippets/from_md/syntax/domains/multi_carrier_domains.aivi{aivi}

## Domains and sigils

Some standard-library domains also expose sigils. For example, a URL domain may offer `~u(https://example.com)` and a path domain may offer `~path[/usr/local/bin]`. These sigils are compile-time validated constructors for typed values, not raw strings.

In v0.1, custom sigils are compiler-provided for standard-library domains only. They are not declared inside a `domain` block.

## Domains are not implicit casts

Domains define operator semantics and literal templates. They do **not** act as a global implicit-conversion system.

The only implicit conversions in v0.1 are expected-type coercions authorized by in-scope instances. See [Types: Expected-Type Coercions](types/expected_type_coercions.md).

## Practical rules of thumb

- Import the domain anywhere you want its operators or suffixes to be active
- Prefer domains when units or operator meaning matter to correctness
- Keep suffix names unambiguous inside one module
- Use explicit constructors if a suffix literal would be unclear to a reader
