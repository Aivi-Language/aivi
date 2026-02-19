# Operators and Context (Operand Index)

This chapter is an index of AIVI's **operator tokens** (and a few pieces of punctuation that act like operators), explaining what each one means **in context**.

> AIVI’s operator *syntax* is fixed (tokens + precedence). Domains can provide **semantics** for some operators when a non-`Int` carrier type is involved (see [Domains](06_domains.md)).

## 11.1 Operator and punctuation index (v0.1)

| Token       | Name | Where it appears | Meaning |
|-------------| --- | --- | --- |
| `=`         | binding | top-level, blocks | Define a value / function clause / type alias. |
| `<-`        | binder | `do Effect {}`, `generate {}`, `resource {}` | Bind each produced value (generator) or run/bind effect/resource results. |
| `->`        | guard | `generate {}` | Filter current generator element using predicate syntax (implicit `_`). |
| `\|>`       | pipe | expressions | Left-to-right application: `x |> f` is `f x`. Chains for readable data transforms. |
| `<\|`       | patch | expressions | Record update: `target <| { field: value }` applies a patch to a record value. |
| `match`     | match / refutable | expressions | The `match` keyword marks refutable pattern matching. Introduces match arms after the scrutinee expression. |
| `           |` | arm / union separator | `match` arms, `type` RHS | Separates match arms and sum-type constructors. |
| `=>`        | arrow | lambdas, match arms, `loop` | Lambda body delimiter and match arm delimiter. |
| `..`        | range | list literals | `a .. b` builds a list of `Int` from `a` to `b` (inclusive). Only valid inside list literals (see 11.3). |
| `...`       | spread / rest | list/record literals, list patterns | Spreads a list/record into another; in patterns, binds the “rest” of a list. |
| `.`         | access / accessor sugar | expressions | Field access `x.field`. Also `.field` is accessor sugar (`x => x.field`). |
| `[]`        | list / index | expressions | List literal `[a, b]`; list range `[a..b]`; index `xs[i]`; bracket-list call `f[a, b]` in specific positions. |
| `{}`        | record / block | expressions, types, patterns | Record literal/type/pattern, patch literal, or block; disambiguated by lookahead (see grammar notes). |
| `()`        | group / tuple / call | expressions, types | Grouping `(e)`, tuple `(a, b)`, call `f(x)`, and suffix application `(e)px`. |
| `~tag[...]` | sigil | literals | Structured literals such as `~path[...]`, `~map{...}`, `~r/.../`. |

For full concrete syntax, see the grammar: [02_syntax/00_grammar.md](00_grammar.md).

## 11.2 Infix operators and precedence (v0.1 surface)

The v0.1 parser recognizes these infix operators (from lowest to highest precedence):

1. `|>` (forward pipe)
2. `??` (coalesce   unwrap `Option` with fallback)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `|` (bitwise or)
8. `^` (bitwise xor)
9. `<<`, `>>` (shift)
10. `+`, `-`, `++` (additive / concatenation)
11. `*`, `×`, `/`, `%` (multiplicative)
12. `<|` (patch   binds tighter than arithmetic, just below application; see [Patching](05_patching.md))

Unary prefix operators (not infix): `!` (not), `-` (negate), `~` (bitwise complement).

Note: `..` is **not** a general infix operator; it is a list-item construct (see 11.3).

Domains may provide semantics for a subset of these operators (see 11.4). Precedence is **not** domain-defined.

## 11.3 Lists: literals, range items, and spread

Inside a list literal, there are three ways to contribute elements:

1. **Item**: `x` contributes one element.
2. **Spread**: `...xs` splices the elements of `xs` into the list.
3. **Range item**: `a..b` is treated like an implicit spread of a range list.

<<< ../snippets/from_md/02_syntax/11_operators/block_01.aivi{aivi}

Notes:

- `a .. b` constructs a list of `Int` values from `a` to `b` **inclusive**. If `b < a`, it produces `[]`.
- `a .. b` is syntactically valid only inside list literals (as a `ListItem` in the grammar). `[a..b]` is the canonical spelling.

## 11.4 Domains and operator meaning

Some operators are **domain-resolved** when operand types are not plain `Int`. In v0.1:

- Domain-resolved (when non-`Int` is involved): `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=`
- Not domain-resolved in v0.1 (built-in): `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`, `??`, `++`

Domain operator resolution is a static rewrite to an in-scope function named like `(+)` or `(<)` (see [Desugaring: Domains and Operators](../04_desugaring/09_domains.md)).

### Within-domain overloads (RHS-typed)

A single domain body may define **multiple entries** for the same operator token, differentiated by their full `LHS -> RHS -> Result` types. The compiler selects among them based on the inferred RHS type after the LHS carrier is resolved (see [Desugaring §9.2](../04_desugaring/09_domains.md#92-rhs-typed-overload-selection)).

**Convention**: `×` is reserved for structural products (matrix-matrix, matrix-vector), while `*` is used for scalar scaling:

<<< ../snippets/from_md/02_syntax/11_operators/block_01.aivi{aivi}


Precedence is **not** domain-defined; `×` and `*` share the same precedence level (`11_operators §11.2`).

## 11.5 Units, suffix literals, and template functions

Suffix literals are **not strings**. They elaborate as applying an in-scope *template function*:

- `10ms` elaborates roughly as `1ms 10`
- `(x)ms` elaborates roughly as `1ms x`

These templates are usually provided by a domain (e.g. `aivi.chronos.duration` defines `1ms`, `1s`, `1min`, `1h`).

<<< ../snippets/from_md/02_syntax/11_operators/block_02.aivi{aivi}

## 11.6 Type coercion (expected-type only)

AIVI does not have global implicit casts. The only implicit conversions in v0.1 are **expected-type coercions** that are explicitly authorized by an in-scope instance (see [Types: Expected-Type Coercions](03_types.md#36-expected-type-coercions-instance-driven)).

Practical consequences:

- No implicit numeric promotion (`Int` does not silently become `Float`).
- Domain operators do not perform implicit unit conversions; conversions are explicit (or modeled as domain operations).
- Coercions are inserted only where an expected type is known (arguments, annotated bindings, record fields under a known type, etc.).

## 11.7 Importing and exporting domains (operator/units scope)

Domains live in modules and are imported/exported explicitly:

- `export domain D` exports the domain `D` from the defining module.
- `use some.module (domain D)` imports only the domain members (operator functions like `(+)` and literal templates like `1ms`) into scope.
- `use some.module` imports the module’s exported values/types; if it exports domains and you want their operator/literal behavior, import the domain explicitly (or rely on the prelude’s default domains).

<<< ../snippets/from_md/02_syntax/11_operators/block_03.aivi{aivi}

Limitations (v0.1):

- Domain names are not values; you cannot write `D.suffix` to qualify suffix templates.
- If two imported domains export the same literal template name (e.g. both define `1m`), the current compiler does not provide carrier-based disambiguation. Prefer selective imports/hiding or explicit constructors to avoid collisions.

