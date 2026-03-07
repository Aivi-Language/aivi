# Operators and Context (Operand Index)

This chapter is an index of AIVI's **operator tokens** (and a few pieces of punctuation that act like operators), explaining what each one means **in context**.

> AIVI’s operator *syntax* is fixed (tokens + precedence). Domains can provide **semantics** for some operators when a non-`Int` carrier type is involved (see [Domains](domains.md)).

## 11.1 Operator and punctuation index (v0.1)

| Token       | Name | Where it appears | Meaning |
|-------------| --- | --- | --- |
| `=`         | binding | top-level, blocks | Define a value / function clause / type alias. |
| `<-`        | binder | `do Effect {}`, `generate {}`, `resource {}` | Bind each produced value (generator) or run/bind effect/resource results. |
| `->`        | guard | `generate {}` | Filter current generator element using predicate syntax (implicit `_`). |
| `\|>`       | pipe | expressions | Left-to-right application: `x \|> f` is `f x`. Chains for readable data transforms. |
| `<\|`       | patch | expressions | Record update: `target <\| { field: value }` applies a patch to a record value. |
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

For full concrete syntax, see the grammar: [syntax/grammar.md](grammar.md).

## 11.2 Infix operators and precedence (v0.1 surface)

The v0.1 parser recognizes these infix operators (from lowest to highest precedence):

1. `|>` (forward pipe)
2. `??` (coalesce   unwrap `Option` with fallback)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `+`, `-`, `++` (additive / collection concatenation)
8. `*`, `×`, `/`, `%` (multiplicative)
9. `<|` (patch   binds tighter than arithmetic, just below application; see [Patching](patching.md))

Unary prefix operators (not infix): `!` (not), `-` (negate).

> **Note:** Bitwise operators (`&`, `|`, `^`, `~`, `<<`, `>>`) are **not** part of AIVI's syntax. Use the [`aivi.bits`](../stdlib/data/bits.md) stdlib module instead.

Note: `..` is **not** a general infix operator; it is a list-item construct (see 11.3).

Domains may provide semantics for a subset of these operators (see 11.4). Precedence is **not** domain-defined.

## 11.3 Lists: literals, range items, and spread

Inside a list literal, there are three ways to contribute elements:

1. **Item**: `x` contributes one element.
2. **Spread**: `...xs` splices the elements of `xs` into the list.
3. **Range item**: `a..b` is treated like an implicit spread of a range list.

<<< ../snippets/from_md/syntax/operators/lists_literals_range_items_and_spread.aivi{aivi}

Notes:

- `a .. b` constructs a list of `Int` values from `a` to `b` **inclusive**. If `b < a`, it produces `[]`.
- `a .. b` is syntactically valid only inside list literals (as a `ListItem` in the grammar). `[a..b]` is the canonical spelling.

## 11.4 Domains and operator meaning

Some operators are **domain-resolved** when operand types are not plain `Int`. In v0.1:

- Domain-resolved (when non-`Int` is involved): `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=`
- Not domain-resolved (always built-in): `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`, `??`
- Domain-resolved via stdlib-registered `(++)` operators: `++` (provided by `aivi.collections` for `List`, `Map`, and `Set`)

> **Note (v0.1):** `++` is supported for `List` concatenation, `Map` merge, and `Set` union. It is **not** supported for `Text` concatenation — use text interpolation (`"Hello, {name}!"`) instead.

Domain operator resolution is a static rewrite to an in-scope function named like `(+)` or `(<)` (see [Domains, Units, and Deltas](domains.md)).

### Within-domain overloads (RHS-typed)

A single domain body may define **multiple entries** for the same operator token, differentiated by their full `LHS -> RHS -> Result` types. The compiler selects among them based on the inferred RHS type after the LHS carrier is resolved (see [Domains §4.2](domains.md#within-domain-overloads-rhs-typed)).

**Convention**: `×` is reserved for structural products (matrix-matrix, matrix-vector), while `*` is used for scalar scaling:

<<< ../snippets/from_md/syntax/operators/within_domain_overloads_rhs_typed.aivi{aivi}


Precedence is **not** domain-defined; `×` and `*` share the same precedence level (`11_operators §11.2`).

## 11.5 Units, suffix literals, and template functions

Suffix literals are **not strings**. They elaborate as applying an in-scope *template function*:

- `10ms` elaborates roughly as `1ms 10`
- `(x)ms` elaborates roughly as `1ms x`

These templates are usually provided by a domain (e.g. `aivi.chronos.duration` defines `1ms`, `1s`, `1min`, `1h`).

<<< ../snippets/from_md/syntax/operators/units_suffix_literals_and_template_functions.aivi{aivi}

## 11.6 Type coercion (expected-type only)

AIVI does not have global implicit casts. The only implicit conversions in v0.1 are **expected-type coercions** that are explicitly authorized by an in-scope instance (see [Types: Expected-Type Coercions](types/expected_type_coercions.md)).

Practical consequences:

- No implicit numeric promotion (`Int` does not silently become `Float`).
- Domain operators do not perform implicit unit conversions; conversions are explicit (or modeled as domain operations).
- Coercions are inserted only where an expected type is known (arguments, annotated bindings, record fields under a known type, etc.).

## 11.7 Importing and exporting domains (operator/units scope)

Domains live in modules and are imported/exported explicitly:

- `export domain D` exports the domain `D` from the defining module.
- `use some.module (domain D)` imports only the domain members (operator functions like `(+)` and literal templates like `1ms`) into scope.
- `use some.module` imports the module’s exported values/types; if it exports domains and you want their operator/literal behavior, import the domain explicitly (or rely on the prelude’s default domains).

<<< ../snippets/from_md/syntax/operators/importing_and_exporting_domains_operator_units_scope.aivi{aivi}

Limitations (v0.1):

- Domain names are not values; you cannot write `D.suffix` to qualify suffix templates.
- If two imported domains export the same literal template name (e.g. both define `1m`), the current compiler does not provide carrier-based disambiguation. Prefer selective imports/hiding or explicit constructors to avoid collisions.

---

## 11.8 Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

<<< ../snippets/syntax/sigils/basic.aivi{aivi}

Domains define sigils to validate and construct types at compile time. Some sigils are compiler-provided and backed by stdlib domains:

- `~u(https://example.com)` / `~url(https://example.com)` → `aivi.url.Url`
- `~path[/usr/local/bin]` → `aivi.path.Path`
- `~r/pattern/flags` → `aivi.regex.Regex`
- `~mat[...]` → matrix literals (`aivi.matrix.Mat2`, `Mat3`, `Mat4`)
- `~d(2024-05-21)` → `Date`, `~t(12:00:00)` → `Time`, `~tz(Europe/Paris)` → `TimeZone`

### Raw text sigil

`` ~`...` `` produces a `Text` value verbatim — no `{ }` interpolation, supports multiple lines:

```aivi
json  = ~`{"id": 1, "name": "Alice"}`
query = ~`SELECT *
          FROM users
          WHERE id = 1`
poem  = ~`
          | Hallo
          | Andreas
`
```

If every non-empty line in a multiline backtick sigil starts with optional indentation followed by `|`, AIVI strips the indentation, strips the `|`, and also drops one optional space after the `|`. A leading blank line immediately after ``~` `` and a trailing blank line immediately before the closing backtick are also removed in that margin mode.

The VSCode extension also recognizes an optional embedded-language header on the first line of a multiline raw-text sigil. When the first line is one of `css`, `html`, `xml`, `json`, `sql`, `js`, `javascript`, `ts`, or `typescript`, that header is metadata only — it is not part of the resulting `Text` value — and the extension injects the matching syntax highlighter into the body:

```aivi
styles = ~`css
  | .myClass {
  |   color: red;
  | }
`
```

Use raw-text sigils when the content contains `{`, `}`, or backslashes that would need escaping in a regular `"..."` string literal.

### Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. The `Collections` domain defines:

<<< ../snippets/syntax/sigils/structured.aivi{aivi}

The `Matrix` domain defines `~mat[...]`; see [Matrix](../stdlib/math/matrix.md).

### HTML and GTK sigils

The UI layer defines two structured sigils:

**`~<html>...</html>`** — typed `aivi.ui.VNode` constructors. Supports `{ expr }` splices. Uppercase/dotted tags are treated as component calls with record-based lowering (`<Ui.Card title="Hello" />` → `Ui.Card { title: "Hello" }`).

**`~<gtk>...</gtk>`** — GtkBuilder-style XML to typed `aivi.ui.gtk4.GtkNode` constructors.

**Shorthand widget tags** — tags starting with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="WidgetName">`. Attributes on shorthand tags become props automatically. Signal sugar (`onClick`, `onInput`, etc.) applies.

```aivi
// Shorthand (preferred)
~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label="Hello" />
    <GtkButton label="Save" onClick={ Msg.Save } />
  </GtkBox>
</gtk>

// Equivalent verbose form
~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="GtkLabel" props={{ label: "Hello" }} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={ Msg.Save } />
  </object>
</gtk>
```

**Signal sugar quick reference:**

```aivi
~<gtk>
  <object class="GtkButton" onClick={ Msg.Save } />
  <object class="GtkEntry" onInput={ Msg.Changed } />
  <object class="GtkButton">
    <signal name="clicked" on={ Msg.Save } />
  </object>
</gtk>
```

Sugar attribute → GTK signal: `onClick`→`clicked`, `onInput`→`changed`, `onActivate`→`activate`, `onToggle`→`toggled`, `onValueChanged`→`value-changed`, `onFocusIn`→`focus-enter`, `onFocusOut`→`focus-leave`.

Signal handlers must be compile-time expressions in v0.1.

**`props` attribute** — `props={ { marginTop: 24, spacing: 24 } }` is sugar that lowers to normalized GTK property entries. Only compile-time record literals are accepted; non-literal values produce a diagnostic.

**Dynamic repeated children** via `<each items={items} as={item}>...</each>` inside GTK elements.

**Uppercase/dotted tags** that are not GTK widget prefixes are treated as component calls — signal sugar and `props` normalization do not apply.

**Diagnostics for GTK sigils:**

| Code | Condition |
|:-----|:----------|
| E1612 | Invalid `props` shape (must be compile-time record literal) |
| E1613 | Non-literal `props` value |
| E1614 | Non-compile-time signal handler binding |

See [Collections](../stdlib/core/collections.md) for `~map` and `~set`, and [HTML Sigil](../stdlib/ui/html.md) for `~html`.
