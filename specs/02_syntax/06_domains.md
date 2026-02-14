# Domains

Domains are AIVI's mechanism for context-aware semantics. They allow the language to adapt to specific problem spaces—like time, geometry, or UI styling—by providing custom interpretations for operators, literals, and type interactions.

Instead of baking specific logic (like "days often have 24 hours but not always") into the core compiler, AIVI delegates this to **Domains**.

## Using Domains

To use a domain, you `use` it. This brings its operators and literals into scope.

<<< ../snippets/from_md/02_syntax/06_domains/block_01.aivi{aivi}

## Units and Deltas

Domains often introduce **Units** (measurements) and **Deltas** (changes).

### Delta Literals (Suffixes)

Deltas represent a relative change or a typed quantity. They are written as numeric literals with a suffix.

<<< ../snippets/from_md/02_syntax/06_domains/block_02.aivi{aivi}

These are **not** strings; they are typed values. `10m` might compile to a `Duration` struct or a float tagged as `Meters`, depending on the active domain.

<<< ../snippets/from_md/02_syntax/06_domains/block_03.aivi{aivi}

### Suffix Application (Variables)

Suffix literals can also be applied to a parenthesized expression, allowing variables and computed values:

<<< ../snippets/from_md/02_syntax/06_domains/block_08.aivi{aivi}

This form requires parentheses and the suffix must be **adjacent** to the closing `)` (write `(x)kg`, not `(x) kg`).

### Domain-Owned Operators (Including `×`)

Domains may define semantics for operators beyond plain numeric arithmetic, including the `×` operator for product/cross-product style operations.


## Defining Domains

You can define your own domains to encapsulate logic. A domain relates a **Carrier Type** (the data) with **Delta Types** (changes) and **Operators**.

### Syntax

<<< ../snippets/from_md/02_syntax/06_domains/block_04.aivi{aivi}

### Example: A Simple Color Domain

<<< ../snippets/from_md/02_syntax/06_domains/block_05.aivi{aivi}

### Interpretation

When you write:

<<< ../snippets/from_md/02_syntax/06_domains/block_06.aivi{aivi}

The compiler sees `red` is type `Rgb`. It looks for a domain over `Rgb` (the `Color` domain). It then desugars `10l` using the domain's rules into `Lightness 10`, and maps `+` to the domain's `(+)` function.

This requires the domain to be in scope (e.g. `use aivi.color (domain Color)`), not just the carrier type.

## Multi-Carrier Domains

Some domains cover multiple types (e.g., `Vector` over `Vec2` and `Vec3`). In v0.1, this is handled by defining the domain multiple times, once for each carrier.

<<< ../snippets/from_md/02_syntax/06_domains/block_07.aivi{aivi}
