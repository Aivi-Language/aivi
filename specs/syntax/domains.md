# Domains

Domains are AIVI's mechanism for context-aware semantics. They allow the language to adapt to specific problem spaces like time, geometry, or UI styling by providing custom interpretations for operators, literals, and type interactions.

Instead of baking specific logic (like "days often have 24 hours but not always") into the core compiler, AIVI delegates this to **Domains**.

## Using Domains

To use a domain, you `use` it. This brings its **operator functions** and **literal templates** into scope.

<<< ../snippets/from_md/syntax/domains/using_domains.aivi{aivi}

### Importing a domain explicitly

Domains are exported/imported separately from normal values and types.

- Export: `export domain Calendar`
- Import domain members: `use aivi.chronos.calendar (domain Calendar)`

Importing a domain brings its members (operator definitions like `(+)` and literal templates like `1d`) into the current scope.

> Domain names are not values; you do not refer to a domain as `Calendar` at term level. You import it to activate its operators and templates.

## Units and Deltas

Domains often introduce **Units** (measurements) and **Deltas** (changes).

### Delta Literals (Suffixes)

Deltas represent a relative change or a typed quantity. They are written as numeric literals with a suffix.

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_01.aivi{aivi}

These are **not** strings; they are typed values. `10m` might compile to a `Duration` struct or a float tagged as `Meters`, depending on the active domain.

Common stdlib suffix examples:

| Suffix | Domain | Type | Module |
| --- | --- | --- | --- |
| `10ms`, `1s`, `5min`, `2h` | Duration | `Duration` | [aivi.chronos.duration](../stdlib/chronos/duration.md) |
| `1d`, `2w`, `3mo`, `1y` | Calendar | `CalendarDelta` | [aivi.chronos.calendar](../stdlib/chronos/calendar.md) |
| `20deg`, `1.2rad` | Angle | `Angle` | [aivi.math](../stdlib/math/math.md) |
| `10l`, `5s`, `30h` | Color | `ColorDelta` | [aivi.color](../stdlib/ui/color.md) |

<<< ../snippets/from_md/syntax/domains/delta_literals_suffixes_02.aivi{aivi}

### Suffix Application (Variables)

Suffix literals can also be applied to a parenthesized expression, allowing variables and computed values:

<<< ../snippets/from_md/syntax/domains/suffix_application_variables.aivi{aivi}

This form requires parentheses and the suffix must be **adjacent** to the closing `)` (write `(x)kg`, not `(x) kg`).

### Domain-Owned Operators (Including `×`)

Domains may define semantics for operators beyond plain numeric arithmetic, including the `×` operator for transform/matrix-multiplication style operations.

### Within-Domain Operator Overloads (RHS-Typed)

A single domain body may contain **multiple entries for the same operator token** (e.g. two `(*)` definitions), provided their full function types differ. The compiler selects among them by matching the inferred RHS type after the LHS carrier is resolved.

<<< ../snippets/from_md/syntax/domains/within_domain_operator_overloads_rhs_typed.aivi{aivi}

**Convention**: Use `×` for structural products (matrix-matrix, matrix-vector), and `*` for scalar scaling. This reserves `×` as the visual signal for "transform-style" multiplication.

Rules:

1. Domain declarations remain `domain D over Carrier`   exactly **one** carrier type per declaration.
2. Multiple operator entries with the **same token** are allowed as long as their full `LHS -> RHS -> Result` types are pairwise distinct.
3. Resolution requires that exactly **one** overload matches the inferred `(LHS, RHS)` pair (see [Desugaring: Domains and Operators](../desugaring/domains.md#92-rhs-typed-overload-selection)).

## Supported operator hooks (v0.1)

In v0.1, the surface language has built-in syntax for operators, and domains may supply semantics for a subset of them.

| Operator | Built-in meaning | Domain meaning (when non-`Int` carrier involved) |
| --- | --- | --- |
| `+`, `-`, `*`, `×`, `/`, `%` | `Int` arithmetic | Resolved to an in-scope operator function like `(+)` |
| `<`, `<=`, `>`, `>=` | `Int` ordering | Resolved to an in-scope operator function like `(<)` returning `Bool` |

Not domain-resolved in v0.1 (always built-in): `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`.

See also: [Operators and Context](operators.md#114-domains-and-operator-meaning).

## Literal templates, suffixes, and collisions

Suffix literals are implemented as template functions named `1{suffix}` that must be in scope:

- `10ms` uses `1ms`
- `(x)ms` uses `1ms`

Domains commonly define these templates in their body (e.g. the `Duration` domain defines `1ms`, `1s`, `1min`, `1h`).

If two imported domains define the same template name (for example, both define `1m`), the current compiler does not provide carrier-based disambiguation. Prefer:

- importing only one of the conflicting domains in a module,
- using selective imports/hiding to avoid bringing conflicting templates into scope, or
- using explicit constructors/functions instead of suffix literals.


## Defining Domains

You can define your own domains to encapsulate logic. A domain relates a **Carrier Type** (the data) with **Delta Types** (changes) and **Operators**.

### Syntax

<<< ../snippets/from_md/syntax/domains/syntax.aivi{aivi}

### Example: A Simple Color Domain

<<< ../snippets/from_md/syntax/domains/example_a_simple_color_domain.aivi{aivi}

### Interpretation

When you write:

<<< ../snippets/from_md/syntax/domains/interpretation.aivi{aivi}

The compiler sees `red` is type `Rgb`. It looks for a domain over `Rgb` (the `Color` domain). It then desugars `10l` using the domain's rules into `Lightness 10`, and maps `+` to the domain's `(+)` function.

This requires the domain to be in scope (e.g. `use aivi.color (domain Color)`), not just the carrier type.

## Multi-Carrier Domains

Some domains cover multiple types (e.g., `Vector` over `Vec2` and `Vec3`). In v0.1, this is handled by defining the domain multiple times, once for each carrier.

<<< ../snippets/from_md/syntax/domains/multi_carrier_domains.aivi{aivi}

## Interaction with Sigils

Domains may define **sigils** (see [Sigils](sigils.md)) that produce domain-typed values. For example, the `Url` domain provides `~u(https://example.com)` and the `Path` domain provides `~path[/usr/local/bin]`. These sigils are validated at compile time and construct typed values, not raw strings.

In v0.1, domains do not support defining custom sigils via the `domain` block. Sigils are compiler-provided for stdlib domains.

## Interaction with type coercion

Domains are not an implicit-cast mechanism. They supply operator semantics and literal templates, but do not introduce global coercions.

The only implicit conversions in v0.1 are expected-type coercions authorized by in-scope instances (see [Types: Expected-Type Coercions](types/expected_type_coercions.md)).
