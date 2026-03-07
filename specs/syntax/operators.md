# Operators and Context

Operators in AIVI are small pieces of syntax with fixed precedence and meaning at the parser level. This page shows what each token does in practice and where domains can supply type-directed behavior.

> AIVI fixes the operator tokens and their precedence. Domains can define what some operators mean for their own carrier types, but they do not change how an expression is parsed.

## 11.1 Operator and punctuation index

| Token | Name | Where it appears | What it does |
|---|---|---|---|
| `=` | binding | top-level, blocks | Defines a value, function clause, or type-level item. |
| `<-` | binder | `do Effect {}`, `generate {}`, `resource {}` | Runs and binds the next produced value from an effect, generator, or resource context. |
| `->` | guard | `generate {}` | Filters the current generator element using predicate syntax. |
| `\|>` | pipe | expressions | Feeds the left value into the expression on the right. |
| `<\|` | patch | expressions | Applies a patch to a record value. |
| `match` | match | expressions | Starts refutable pattern matching on the expression to its left. |
| `\|` | arm / union separator | `match` arms, sum-type definitions | Separates branches or constructors. |
| `=>` | arrow | lambdas, match arms, `loop` | Separates inputs or patterns from the resulting expression. |
| `..` | range | list literals | Builds an inclusive `Int` range inside a list literal. |
| `...` | spread / rest | list and record literals, list patterns | Spreads values into a collection or captures the remainder of a list pattern. |
| `.` | access / accessor sugar | expressions | Accesses a field (`x.field`) or builds an accessor function (`.field`). |
| `[]` | list / indexing | expressions | Builds lists, expresses list ranges, or indexes collections where supported. |
| `{}` | record / block | expressions, types, patterns | Forms records, patches, and block syntax depending on context. |
| `()` | grouping / tuple | expressions, types | Groups expressions, builds tuples, and supports suffix literals like `(x)ms`. |
| `~tag[...]` | sigil | literals | Introduces custom literal syntaxes such as regexes, paths, and structured UI literals. |

For the full syntax grammar, see the [grammar reference](grammar.md).

## 11.2 Infix operators and precedence

The parser reads infix operators from lowest to highest precedence in this order:

1. `|>` (forward pipe)
2. `??` (coalesce)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `+`, `-`, `++` (addition / subtraction / concatenation)
8. `*`, `×`, `/`, `%` (multiplication family)
9. `<|` (patch)

Unary prefix operators: `!` for logical negation and `-` for numeric negation.

```aivi
total = subtotal + tax * rate // `*` binds tighter than `+`
```

> **Note:** Bitwise operators such as `&`, `|`, `^`, `~`, `<<`, and `>>` are not language operators. Use the [`aivi.bits`](../stdlib/data/bits.md) standard library module instead.

`..` is not a general infix operator. It is a list-item form that appears only inside list literals.

## 11.3 Lists: literals, range items, and spread

Inside a list literal, there are three ways to contribute elements:

1. **Item**: `x` contributes one element.
2. **Spread**: `...xs` inserts all elements from `xs`.
3. **Range item**: `a..b` expands to the inclusive list of `Int` values from `a` to `b`.

<<< ../snippets/from_md/syntax/operators/lists_literals_range_items_and_spread.aivi{aivi}

Notes:

- `a .. b` produces `[]` when `b < a`.
- The canonical spelling is `[a..b]` because the range form belongs to list syntax.

## 11.4 Domains and operator meaning

Some operators are built in, and some are resolved through imported domains when the operand types require it.

In AIVI:

- domain-resolved when a non-`Int` carrier is involved: `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=`
- always built in: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`, `??`
- domain-resolved through registered `(++)` bindings: `++`

> **Note:** `++` is used for `List` concatenation, `Map` merge, and `Set` union. Use text interpolation for `Text` concatenation.

Domain operator resolution is a static rewrite to an in-scope function such as `(+)` or `(<)`.

### Within-domain overloads (RHS-typed)

A domain may define several entries for the same operator token as long as their full types differ. After the compiler knows the left-hand carrier, it can use the right-hand type to choose the correct operator definition.

**Convention:** use `×` for structural products such as matrix-matrix or matrix-vector multiplication, and use `*` for scalar scaling.

<<< ../snippets/from_md/syntax/operators/within_domain_overloads_rhs_typed.aivi{aivi}

Precedence is still fixed by the language, so `×` and `*` share the same precedence level.

## 11.5 Units, suffix literals, and template functions

Suffix literals are not strings. They elaborate to an in-scope template function.

Typical examples:

- `10ms` elaborates roughly to `1ms 10`
- `(x)ms` elaborates roughly to `1ms x`

These templates are usually provided by domains such as a duration domain.

<<< ../snippets/from_md/syntax/operators/units_suffix_literals_and_template_functions.aivi{aivi}

## 11.6 Type coercion (expected-type only)

AIVI does not do general implicit casting. The only implicit conversions are **expected-type coercions** authorized by an in-scope instance.

Practical consequences:

- `Int` does not silently become `Float`
- domain operators do not silently convert units
- coercions are inserted only where an expected type is already known, such as function arguments or annotated fields

See [Types: Expected-Type Coercions](types/expected_type_coercions.md).

## 11.7 Importing and exporting domains (operator/units scope)

Domains live inside modules and must be imported or exported explicitly.

- `export domain D` publishes the domain from its module
- `use some.module (domain D)` imports the domain members such as operator bindings and literal templates
- plain `use some.module` imports exported values and types, but does not automatically activate the domain behavior

<<< ../snippets/from_md/syntax/operators/importing_and_exporting_domains_operator_units_scope.aivi{aivi}

Limitations:

- domain names are not first-class values
- if two imported domains export the same template name, the compiler does not disambiguate by carrier type; prefer selective imports, hiding, or explicit constructors

---

## 11.8 Sigils

Sigils are custom literal syntaxes for values that are awkward to write as plain text or records.

<<< ../snippets/syntax/sigils/basic.aivi{aivi}

Domains define many sigils. Some are compiler-provided and backed by standard-library domains:

- `~u(https://example.com)` / `~url(https://example.com)` → `aivi.url.Url`
- `~path[/usr/local/bin]` → `aivi.path.Path`
- `~r/pattern/flags` → `aivi.regex.Regex`
- `~mat[...]` → matrix literals such as `aivi.matrix.Mat2`, `Mat3`, `Mat4`
- `~d(2024-05-21)` → `Date`, `~t(12:00:00)` → `Time`, `~tz(Europe/Paris)` → `TimeZone`

### Raw text sigil

`` ~`...` `` produces `Text` verbatim. Use it when ordinary string escaping would get noisy.

```aivi
json  = ~`{"id": 1, "name": "Alice"}` // braces stay literal
query = ~`SELECT *
          FROM users
          WHERE id = 1`
poem  = ~`
          | Hallo
          | Andreas
`
```

If every non-empty line in a multiline backtick sigil starts with optional indentation followed by `|`, AIVI strips the indentation, strips the `|`, and also drops one optional space after the `|`. A leading blank line immediately after ``~` `` and a trailing blank line immediately before the closing backtick are also removed in that margin mode.

The VSCode extension also recognizes an optional embedded-language header on the first line of a multiline raw-text sigil. When the first line is one of `css`, `html`, `xml`, `json`, `sql`, `js`, `javascript`, `ts`, or `typescript`, that header is metadata only and the extension injects matching syntax highlighting into the body.

```aivi
styles = ~`css
  | .myClass {
  |   color: red;
  | }
`
```

### Structured sigils

Some sigils are parsed as AIVI expressions rather than raw text.

The `Collections` domain defines:

<<< ../snippets/syntax/sigils/structured.aivi{aivi}

The `Matrix` domain defines `~mat[...]`; see [Matrix](../stdlib/math/matrix.md).

### HTML and GTK sigils

The UI layer defines two structured sigils:

- `~<html>...</html>` builds typed `aivi.ui.VNode` values and supports `{ expr }` splices
- `~<gtk>...</gtk>` builds typed `aivi.ui.gtk4.GtkNode` values from GtkBuilder-style XML

Uppercase or dotted tags in HTML sigils are treated as component calls with record-based lowering.

**Shorthand widget tags** starting with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="WidgetName">` in GTK sigils.

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

Sugar attribute → GTK signal: `onClick` → `clicked`, `onInput` → `changed`, `onActivate` → `activate`, `onToggle` → `toggled`, `onValueChanged` → `value-changed`, `onFocusIn` → `focus-enter`, `onFocusOut` → `focus-leave`.

Signal handlers must be compile-time expressions.

**`props` attribute** is sugar for a compile-time record literal of GTK properties. Non-literal values produce a diagnostic.

**Dynamic repeated children** are written with `<each items={items} as={item}>...</each>` inside GTK elements.

Uppercase or dotted tags that are not GTK widget prefixes are treated as component calls, so GTK-specific signal sugar and `props` normalization do not apply.

**Diagnostics for GTK sigils:**

| Code | Condition |
|---|---|
| E1612 | Invalid `props` shape (must be a compile-time record literal) |
| E1613 | Non-literal `props` value |
| E1614 | Non-compile-time signal handler binding |

See [Collections](../stdlib/core/collections.md) for `~map` and `~set`, and [HTML Sigil](../stdlib/ui/html.md) for `~html`.
