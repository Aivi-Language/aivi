# Domains

Domains let AIVI give familiar syntax a domain-specific meaning for a type. Instead of teaching the core language every rule about calendars, units, geometry, or colors, a domain says how operators and suffix literals should behave for a particular kind of value.

## What a domain is

A domain is a bundle of semantics for a **carrier type** such as a date, duration, vector, or color. A domain may provide:

- operator meanings such as `+`, `-`, `*`, or `×`
- typed suffix literals such as `10ms`, `2h`, or `10l`
- helper types for changes or measurements, often called **deltas** — typed changes such as "three days", "five meters", or "twenty degrees"
- compile-time validation for domain-owned syntax

That means code can stay concise without becoming untyped. For example, `30d` is not a string; it is a typed value supplied by an active domain.

## What domains are for

Domains are useful when plain numbers are not enough. Reach for them when you want:

- **typed units** instead of “magic numbers”
- **readable operators** for a specific problem space
- **literal suffixes** that compile into real values
- **clear separation** between the core language and specialized rules

Typical examples include time and calendar arithmetic, geometry, matrices, UI layout units, and color adjustment.

## Using an existing domain

To use a domain, bring the domain itself into scope. The most explicit form is:

<<< ../snippets/from_md/syntax/domains/block_01.aivi{aivi}


<<< ../snippets/from_md/syntax/domains/block_02.aivi{aivi}

::: repl
```aivi
/use aivi.chronos.duration
gap = 2h + 30m
// => Duration: 2h 30m
```
:::

The first example uses a module that exports its domain directly. The second shows a companion-module pattern (a separate module that provides extra operations for a domain type) used by some standard-library areas: one module provides the named helpers, and another exports the domain sugar.

### Importing a domain explicitly

Domains are separate import items.

- Export a domain with `export domain Color`
- Import it selectively with `use aivi.color (Rgb, domain Color)`
- A plain `use aivi.color` can also bring the exported domain into scope when you want the module's whole public API

Importing the domain activates its members, including operator definitions like `(+)` and literal templates like `1l`.

> Domain names are not runtime values. You do not pass `Color` around as a function argument; you import the domain so its syntax becomes available in the current module.

## Typed deltas and suffix literals

Domains often introduce **delta** values: typed changes such as “three days”, “five meters”, or “twenty degrees”. These are commonly written as numeric literals with a suffix.

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_01.aivi{aivi}

These literals are fully typed. The exact result type depends on the template behind the suffix: many suffixes produce a delta value, while some produce a carrier value directly.

Common standard-library suffixes include:

| Suffix | Domain | What it builds | Module |
| --- | --- | --- | --- |
| `10ms`, `1s`, `5min`, `2h` | Duration | time-span deltas used with `Span` arithmetic | [aivi.chronos.duration](../stdlib/chronos/duration.md) |
| `1d`, `3m`, `1y` | Calendar | calendar deltas used with `Date` arithmetic | [aivi.chronos.calendar](../stdlib/chronos/calendar.md) |
| `10px`, `2em`, `50%` | Layout | typed UI units such as `Length` and `Percentage` | [aivi.ui.layout](../stdlib/ui/layout.md) |
| `10l`, `5s`, `30h`, `8r` | Color | HSL-style deltas (`l`, `s`, `h`) and channel values (`r`, `g`, `b`) | [aivi.color](../stdlib/ui/color.md) |

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_02.aivi{aivi}

Angles are typed too, but today they use constructors such as `degrees 90.0` and `radians pi` from [`aivi.math`](../stdlib/math/math.md) rather than a standard-library domain suffix.

## Applying suffixes to computed values

You can also attach a suffix to a parenthesized expression. This is the form to use when the numeric part is stored in a variable or computed first.

<<< ../snippets/from_md/syntax/domains/suffix_application_variables.aivi{aivi}

Write the suffix immediately after the closing `)`:

- correct: `(x)kg`
- incorrect: `(x) kg`

## Domain-owned operators

Domains may define operator behavior that goes beyond plain integer arithmetic. That includes familiar operators such as `+` and `*`, and also `×` for transform-style multiplication (such as matrix-by-matrix or matrix-by-vector products).

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

In v0.1, domains can participate in the operator syntax below when an in-scope definition matches the operand types.

| Operator family | Notes |
| --- | --- |
| `+`, `-`, `*`, `×`, `/`, `%` | Fall back to built-in numeric behavior for `Int` and `Float`, or resolve to an in-scope domain operator such as `(+)`. |
| `++` | May be supplied by an in-scope domain or module export for collection-like carriers. |
| `<`, `<=`, `>`, `>=` | Keep built-in ordering for `Int`, `Float`, and `Text`, or resolve to an in-scope domain operator such as `(<)` that returns `Bool`. |

These operators are **not** domain-resolved in v0.1 and always keep their built-in meaning: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`, `??`.

See also: [Operators and Context](operators.md#114-domains-and-operator-meaning).

## Literal templates and name collisions

Suffix literals are implemented through template functions named `1{suffix}`:

- `10ms` uses `1ms`
- `(x)ms` also uses `1ms`

Domains usually define these templates in the domain body. The template signature determines which numeric forms are accepted, for example `1l : Int -> Delta` or a custom floating-point template such as `1turn : Float -> Angle`.

If two imported domains define the same template name, the compiler does not currently disambiguate them by carrier type. In practice, avoid collisions by:

- importing only one conflicting domain in a module
- using selective imports or hiding
- calling an explicit constructor or helper function instead of using the suffix literal

## Defining your own domain

Define a domain when you want problem-specific syntax that stays type-safe and predictable.

### Syntax

<<< ../snippets/from_md/syntax/domains/block_03.aivi{aivi}


### Example: a simple color domain

<<< ../snippets/from_md/syntax/domains/block_04.aivi{aivi}


### How the compiler reads a domain expression

When you write a domain-owned expression, the compiler starts from the carrier type and then resolves the operator and any suffix literals through the domain currently in scope.

<<< ../snippets/from_md/syntax/domains/block_05.aivi{aivi}


For a value like `red + 10l`, the flow is:

1. infer that `red` has the carrier type, for example `Rgb`
2. find an in-scope domain declared over that carrier, for example `Color`
3. desugar `10l` using the domain’s template for that suffix
4. resolve `+` to the domain’s `(+)` definition

This only works when the domain itself is in scope. `use aivi.color (Rgb)` is not enough; write `use aivi.color (Rgb, domain Color)` or import the whole module if you want all of its exported names and domain behavior.

## Multi-carrier domains

Sometimes one idea applies to several carrier types, such as both `Vec2` and `Vec3`. In v0.1, write one domain declaration per carrier type.

<<< ../snippets/from_md/syntax/domains/multi_carrier_domains.aivi{aivi}

Reusing the same domain name across those declarations is normal. Resolution still starts from the carrier type on the left-hand side of the operator.

## Domains are not implicit casts

Domains define operator semantics and literal templates. They do **not** act as a global implicit-conversion system.

The only implicit conversions in v0.1 are expected-type coercions authorized by in-scope instances. See [Types: Expected-Type Coercions](types/expected_type_coercions.md).

## Practical rules of thumb

- Import the domain anywhere you want its operators or suffixes to be active
- Prefer domains when units or operator meaning matter to correctness
- Keep suffix names unambiguous inside one module
- Use explicit constructors if a suffix literal would be unclear to a reader
