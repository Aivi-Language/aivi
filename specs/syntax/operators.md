# Operators and Context

Operators in AIVI are small pieces of syntax with fixed precedence and meaning at the parser level. For example, `subtotal + tax` is always parsed as `+`, but a domain can decide what `+` means for a type such as `Distance` or `Money`. This page shows what each token does in practice and where domains can supply type-directed behavior.

> AIVI fixes the operator tokens and their precedence. Domains can define what some operators mean for their own carrier types, but they do not change how an expression is parsed.

## 11.1 Operator and punctuation index

| Token | Name | Where it appears | What it does |
|---|---|---|---|
| `=` | binding | top-level, blocks | Defines a value, function clause, or type-level item. |
| `<-` | binder | `do Effect {}`, `generate {}`, `resource {}` | Runs and binds the next produced value from an effect, generator, or resource context. |
| `->` | guard | `generate {}` | Filters the current generator element using predicate syntax. |
| `\|>` | pipe | expressions | Feeds the left value into the expression on the right. |
| `->>` | signal derive | expressions | Derives a new signal from a source signal by applying a transform to the current value. The left side must be a `Signal A`. |
| `<\|` | patch | expressions | Applies a patch to a record value. |
| `<<-` | signal write | expressions | Writes a signal with a value, updater function, or record patch. The left side must be a `Signal A`. |
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

1. `|>` (forward pipe) / `->>`  (signal derive) — same precedence
2. `??` (coalesce)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `+`, `-`, `++` (addition / subtraction / concatenation)
8. `*`, `×`, `/`, `%` (multiplication family)
9. `<|` (patch) / `<<-` (signal write) — same precedence

Unary prefix operators: `!` for logical negation and `-` for numeric negation.
Within one precedence level, infix operators associate left-to-right. Add parentheses when you want a different grouping.

<<< ../snippets/from_md/syntax/operators/block_01.aivi{aivi}


> **Note:** Bitwise operators such as `&`, `|`, `^`, `~`, `<<`, and `>>` are not language operators. Use the [`aivi.bits`](../stdlib/data/bits.md) standard library module instead.

`..` is not a general infix operator. It is a list-item form that appears only inside list literals.

### Patch application (`<|`)

On ordinary data, `<|` applies a patch literal to the value on its left and returns a new value. It never mutates the original.

<<< ../snippets/from_md/syntax/operators/block_02.aivi{aivi}


See [Patching Records](patching.md) for reusable `patch { ... }` values, deep selectors, and collection-aware updates.

### Signal write (`<<-`)

`<<-` writes to a signal. The left side must be a `Signal A`. The semantics depend on the right-hand side:

- `signal <<- value` → `set signal value`
- `signal <<- updater` → `update signal updater`
- `signal <<- { ... }` → update the current record value with the same patch semantics as `<|`

```aivi
count <<- 10
profile <<- (state => { name: "AIVI", saveCount: state.saveCount + 1 })
profile <<- { saveCount: _ + 1 }
```

### Signal derivation (`->>`)

`signal ->> fn` is shorthand for `derive signal (value => value |> fn)`, so the right-hand side still uses the normal pipe rules over the current signal value. That means the RHS can be an expression, lambda sugar, or a bare matcher block. The left side must be a `Signal A`.

```aivi
count = signal 1
countText = count ->> (_ + 1) ->> toText
label = count ->> (n => "Count {n}")
aiSettingsOpen = shellState ->>
  | Some AiSettingsSection => True
  | _                      => False
```

## 11.3 Lists: literals, range items, and spread

Inside a list literal, there are three ways to contribute elements:

1. **Item**: `x` contributes one element.
2. **Spread**: `...xs` inserts all elements from `xs`.
3. **Range item**: `a..b` expands to the inclusive list of `Int` values from `a` to `b`.

<<< ../snippets/from_md/syntax/operators/lists_literals_range_items_and_spread.aivi{aivi}

::: repl
```aivi
xs = [1..5]
// => [1, 2, 3, 4, 5]
ys = [0, ...xs, 6]
// => [0, 1, 2, 3, 4, 5, 6]
```
:::

Notes:

- `a .. b` produces `[]` when `b < a`.
- The canonical spelling is `[a..b]` because the range form belongs to list syntax.

## 11.4 Domains and operator meaning

Some operators are built in, and some are resolved through imported domains when the operand types require it.

In AIVI:

- domain-resolved (operators whose behavior depends on the types of their operands, resolved through the active domain) when a non-`Int` carrier is involved: `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=`
- always built in: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`, `??`
- domain-resolved through registered `(++)` bindings: `++`

> **Note:** `++` is used for `List` concatenation, `Map` merge, and `Set` union. Use text interpolation for `Text` concatenation.

Domain operator resolution is a static rewrite to an in-scope function such as `(+)` or `(<)`.

### Within-domain overloads (RHS-typed (right-hand-side-typed))

A domain may define several entries for the same operator token as long as their full types differ. After the compiler knows the left-hand carrier, it can use the right-hand type to choose the correct operator definition.

**Convention:** use `×` for structural products such as matrix-matrix or matrix-vector multiplication, and use `*` for scalar scaling.

<<< ../snippets/from_md/syntax/operators/within_domain_overloads_rhs_typed.aivi{aivi}

Precedence is still fixed by the language, so `×` and `*` share the same precedence level.

## 11.5 Units, suffix literals, and template functions

A **suffix literal** is a number with a unit-like suffix, such as `10ms`, `5km`, or `100%`.

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

Sigils are named literal forms for values that would be awkward to write as plain text or records.

<<< ../snippets/syntax/sigils/basic.aivi{aivi}

Some standard-library modules expose compiler-provided sigils:

- `~u(https://example.com)` / `~url(https://example.com)` → [`Url`](../stdlib/system/url.md)
- `~path[/usr/local/bin]` → [`Path`](../stdlib/system/path.md)
- `~r/pattern/flags` → [`Regex`](../stdlib/core/regex.md)
- `~mat[...]` → matrix literals such as [`Mat2`, `Mat3`, `Mat4`](../stdlib/math/matrix.md)
- `~d(2024-05-21)` → `Date`, `~t(12:00:00)` → `Time`, `~tz(Europe/Paris)` → `TimeZone`

Use these sigils when the full value is known at compile time. When the source text only arrives at runtime, prefer the module's parser or constructor APIs instead.

### Raw text sigil

`` ~`...` `` produces `Text` verbatim. Use it when ordinary string escaping would get noisy.

<<< ../snippets/from_md/syntax/operators/block_02.aivi{aivi}


If every non-empty line in a multiline backtick sigil starts with optional indentation followed by `|`, AIVI strips the indentation, strips the `|`, and also drops one optional space after the `|`. A leading blank line immediately after ``~` `` and a trailing blank line immediately before the closing backtick are also removed in that margin mode.

The VSCode extension also recognizes an optional embedded-language header on the first line of a multiline raw-text sigil. When the first line is one of `css`, `html`, `xml`, `json`, `sql`, `js`, `javascript`, `ts`, or `typescript`, that header is metadata only and the extension injects matching syntax highlighting into the body.

<<< ../snippets/from_md/syntax/operators/block_03.aivi{aivi}


### Structured sigils

Some sigils are parsed as AIVI expressions rather than raw text.

The `Collections` domain defines:

<<< ../snippets/syntax/sigils/structured.aivi{aivi}

The `Matrix` domain defines `~mat[...]`; see [Matrix](../stdlib/math/matrix.md).

### HTML and GTK sigils

If you only need the headline idea, read this section as: `~<html>` builds typed UI tree values, and `~<gtk>` builds typed GTK node values. The UI reference pages go deeper into runtime behaviour; this section focuses on how to read the syntax.

The UI layer defines two structured sigils:

- `~<html>...</html>` builds typed `aivi.ui.VNode` values and supports `{ expr }` splices
- `~<gtk>...</gtk>` builds typed `aivi.ui.gtk4.GtkNode` values from GtkBuilder-style XML

Uppercase or dotted tags in HTML sigils are treated as component calls with record-based lowering.

**Shorthand widget tags** starting with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="WidgetName">` in GTK sigils.

<<< ../snippets/from_md/syntax/operators/block_04.aivi{aivi}


**Signal sugar quick reference:**

<<< ../snippets/from_md/syntax/operators/block_05.aivi{aivi}


Sugar attribute → GTK signal: `onClick` → `clicked`, `onInput` → `changed`, `onActivate` → `activate`, `onKeyPress` → `key-pressed`, `onToggle` → `notify::active` for `GtkSwitch` and `toggled` elsewhere, `onSelect` → `notify::selected` for `GtkDropDown`, `onClosed` → `closed` for dialog widgets, `onValueChanged` → `value-changed`, `onFocusIn` → `focus-enter`, `onFocusOut` → `focus-leave`, `onShowSidebarChanged` → `notify::show-sidebar` for `AdwOverlaySplitView`.

For controller-only signals that have no sugar, or when you want the raw `GtkSignalEvent`, use explicit signal nodes. Example: attach `GtkEventControllerMotion` under `<child type="controller">` and bind `<signal name="enter" ... />` / `<signal name="leave" ... />`.

The signal name comes from the sugar attribute (or the explicit `name="..."` on `<signal ... />`), but the handler itself is an ordinary AIVI expression inside `{ ... }`: pass a runtime function or an `Event` handle, not a special compile-time-only form.

**`props` attribute** is sugar for a record literal written inside a splice, for example `props={ { label: "Save" } }`. The shape must be a record literal with simple field names, but field values may still be ordinary expressions.

**Dynamic repeated children** are written with `<each items={items} as={item}>...</each>` inside GTK elements.

Uppercase or dotted tags that are not GTK widget prefixes are treated as component calls, so GTK-specific signal sugar and `props` normalization do not apply.

GTK sigils also allow **function-call tags** for local helper functions: `<NavRailNode arg0 arg1 />` lowers to `{ navRailNode arg0 arg1 }`. This sugar only applies to simple non-widget tags, uses positional arguments, and must stay self-closing.

**Diagnostics for GTK sigils:**

| Code | Condition |
|---|---|
| E1612 | Invalid `props` shape (must use a `props={ ... }` splice whose value is a record literal with simple field names) |
| E1614 | Non-compile-time signal handler binding |
| E1615 | Invalid `<each>` usage |
| E1616 | Bare `<child>` without a `type=` attribute |
| E1617 | Invalid GTK function-call tag usage |

See [Collections](../stdlib/core/collections.md) for `~map` and `~set`, [HTML Sigil](../stdlib/ui/html.md) for `~html`, [GTK4 UI](../stdlib/ui/gtk4.md) for GTK runtime details, plus [URL](../stdlib/system/url.md), [Path](../stdlib/system/path.md), and [Regex](../stdlib/core/regex.md) for domain-specific sigils.
