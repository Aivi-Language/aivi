# AIVI

> Compact parser-aligned reference. This file prioritizes the current parser and live `.aivi` code; some RFC/manual examples still show older spellings.

## Core model

- Pure `val` / `fun`; effects cross through `Task`, `sig`, `@source`, and GTK/runtime host boundaries.
- Closed types, exhaustive matches, no `null` / `undefined`, no `if` / `else`, no loops.
- Pipe-first control flow, first-class `Signal`, typed decoding, native GTK/libadwaita UI.
- Style: prefer one flat left-to-right pipe spine. Deeply nested calls or nested pipe chains are an anti-pattern unless a grouped subexpression is genuinely clearer.

## Top-level forms

| Form | Meaning |
| --- | --- |
| `use mod (name, other as alias)` | explicit imports only; no wildcards |
| `export name` | export binding; `main` is the conventional app entry; `view` is the usual preview root |
| `type T = A \| B` | closed sum |
| `type Vec2 = Vec2 Int Int` | constructor product |
| `type User = { name: Text }` | closed record |
| `class Eq A` / `instance Eq User` | type classes / instances |
| `val x[:T] = expr` | immutable value |
| `fun f:Ret x:T y:U => expr` | pure function; current parser uses bare `param:Type` |
| `sig s = expr` | derived signal |
| `sig input:Signal T` | bodyless input signal |
| `provider my.source ...` | custom source provider |
| `@source ...`, `@recur.timer ...`, `@recur.backoff ...` | decorators on the next declaration |

Comments: `//`, `/* ... */`, `/** ... **/`.

Older docs may still show `#x:T`; treat that as outdated syntax drift, not the current parser surface.

## Types, literals, expressions

- Built-ins: `Int`, `Float`, `Decimal`, `BigInt`, `Bool`, `Text`, `Unit`, `Bytes`, `List A`, `Map K V`, `Set A`, `Option A`, `Result E A`, `Validation E A`, `Signal A`, `Task E A`.
- Function types: `A -> B`; higher-kinded constructors such as `Option`, `Result E`, `Task E`, `Signal`.
- Literals: `42`, `3.14`, `19.25d`, `123n`, `True`, `False`, `"hi"`, `rx"\d+"`, `()`, `[1, 2]`, `(1, 2)`, `{ name: "Ada" }`, `Map { "x": 1 }`, `Set [1, 2]`, `[1 .. 10]`.
- Strings use interpolation, not core concatenation: `"Hello {name}"`.
- Function application is juxtaposition: `f x y`; parentheses group only.
- Lambdas: `x => body`; operator sections: `_ + 1`.
- Field access: `user.name`.
- Record shorthand: `{ name, age }`; default elision works only for expected closed records with `Default` evidence.

## Control flow + pipes

- Pattern match:

```aivi
value
 ||> Pattern => expr
 ||> _ => fallback
```

- Pattern forms: wildcard `_`, literals, constructors, record subsets `{ field }`, list patterns `[a, b, ...rest]`, and nesting.
- Ordinary precedence: application > `+ -` > comparisons > `and` > `or`; `not` is prefix; pipe stages parse outside that table.
- Ambient subject in pipe expressions: bare `.` or `_`; ambient projection: `.field` or `.field.subfield`.
- Branch shortcuts use the branch payload as the ambient subject:
  - `maybeUser T|> .name F|> "guest"`
  - `loaded T|> greet . F|> showError .`
  - `loaded T|> greet _ F|> showError _`
- `T|>` / `F|>` are the `if`/`else` replacement for canonical pairs (`Bool`, `Option`, `Result`, `Validation`) and must appear as an adjacent pair in one pipe spine.
- Inside `*|>`, `?|>` acts like a filter and drops items whose predicate is false.

| Operator | Meaning |
| --- | --- |
| `|>` | transform |
| `?|>` | gate / filter |
| `||>` | case split |
| `T|>` / `F|>` | canonical truthy / falsy branch |
| `*|>` | map / fan-out |
| `<|*` | explicit join after `*|>` |
| `|` | tap / observe without changing the subject |
| `&|>` | applicative cluster (`&|> a`, `&|> b`, `|> f`) |
| `@|>` | enter explicit recurrence |
| `<|@` | recurrence step |

## Reactivity, sources, effects

- `sig` is the reactive boundary; `val` must not depend on signals.
- Stateful accumulation uses `scan`: `upstream |> scan seed step` where `step : Event -> State -> State`.
- `&|>` combines independent `Signal` / `Option` / `Result` / `Validation` / `Task` values applicatively; without a finalizer it defaults to a tuple.
- `@source provider.variant args with { ... }` may decorate only a bodyless `sig`.
- Built-in provider families include `timer.*`, `window.*`, `http.*`, `fs.*`, `process.*`, `env.*`, `path.*`, `socket.*`, `mailbox.*`, and `dbus.*`.
- Decode / delivery surfaces include `Strict` / `Permissive` and `Ignore` / `Lines` / `Bytes`.
- `@recur.timer expr`, `@recur.backoff expr`, and `seed @|> start <|@ step` cover explicit recurrence, polling, and retry surfaces.
- `Task E A` is the one-shot effect carrier; `Signal` is applicative, not monadic.

## Markup / GTK surface

- Markup lowers directly to GTK/libadwaita widgets; no virtual DOM.
- Root view must be a `Window`.
- Tags are PascalCase widget names: `<Window>`, `<Box>`, `<Label>`, `<Button />`, etc.
- Attribute expressions use `{...}`; strings inside attributes also support interpolation.
- Event attributes are schema-backed and route only to bodyless input signals, e.g. `onClick={clicked}`.
- Control nodes:
  - `<show when={cond}>...</show>`
  - `<each of={items} as={item} key={item.id}>...<empty>...</empty></each>`
  - `<match on={value}><case pattern={Paid}>...</case></match>`
  - `<fragment>...</fragment>`
  - `<with value={expr} as={name}>...</with>`
- The current live widget/event surface is partial; the documented core catalog centers on `Window`, `Box`, `ScrolledWindow`, `Label`, `Button`, `Entry`, and `Switch`, with host attrs such as `trackVisible={sig}` and `hideOnClose={True}`.

## Domains

- Domains are nominal zero-cost carriers: `domain Name over Carrier`.
- A domain body can declare:
  - `literal ms : Int -> Duration`
  - smart constructors/parsers like `parse : Text -> Result PathError Path`
  - operators like `(+)` or `(/)`
  - `value : Domain -> Carrier` to unwrap explicitly
- Domains do **not** inherit carrier operators automatically, and there are no implicit casts to or from the carrier.
- Common current domain surfaces:
  - `Duration over Int` with literals such as `120ms`, `5sec`, `2min`
  - `Path over Text` with `parse`, `value`, and `(/)`
  - `NonEmpty A over List A` with `fromList`, `head`, and `tail`
  - `Retry over Int` for retry counts such as `3x` in current docs/examples

```aivi
domain Duration over Int
    literal ms  : Int -> Duration
    literal sec : Int -> Duration
    (+)         : Duration -> Duration -> Duration
    value       : Duration -> Int
```

## Basic stdlib

Built-in / ambient names you use constantly: `Option`, `Some`, `None`, `Result`, `Ok`, `Err`, `True`, `False`, `reduce`, `append`, `head`, `tail`.

Common modules:

| Module | Common names |
| --- | --- |
| `aivi.list` | `length`, `head`, `tail`, `last`, `zip`, `any`, `all`, `count`, `find`, `findMap`, `partition`, `isEmpty`, `nonEmpty` |
| `aivi.option` | `isSome`, `isNone`, `getOrElse`, `orElse`, `flatMap`, `flatten`, `toList`, `toResult` |
| `aivi.result` | `withDefault`, `mapOk`, `mapErr`, `toOption`, `fromOption` |
| `aivi.text` | `join`, `concat`, `surround`, `isEmpty`, `nonEmpty` |
| `aivi.fs` | `writeText`, `writeBytes`, `createDirAll`, `deleteFile`, plus `FsError` / `FsEvent` |
| `aivi.stdio` | `stdoutWrite`, `stderrWrite` |
| `aivi.path` | `Path`, `PathError` |
| `aivi.defaults` | `Option` default evidence for record-field elision |

Import explicitly:

```aivi
use aivi.text (join)
use aivi.option (getOrElse)
```

## Classes, validation, providers

- Constraint syntax: `class Eq A => Ord A` or `class (C1 A, C2 A) => X A`.
- Ambient/prelude classes include `Eq`, `Default`, `Ord`, `Semigroup`, `Monoid`, `Functor`, `Bifunctor`, `Applicative`, `Monad`, `Foldable`, `Traversable`, and `Filterable`.
- `Validation` is applicative-first and is the canonical `&|>` form-validation carrier.

Custom providers:

```aivi
provider my.data.source
    wakeup: providerTrigger
    argument url: Url
    option timeout: Duration
```

## Current notable limits

- User-defined imported instances are deferred; same-module instance bodies are the implemented surface.
- `@source` only attaches to bodyless `sig`.
- Markup handlers must be direct input signals, not arbitrary callback expressions.
- `Signal` is `Functor` / `Applicative`, not `Monad`; `Validation` is applicative-only.
- Parts of the runtime / GTK / tooling surface are still partial, so when docs disagree, the parser and current `.aivi` code are the safer source of truth.

## Tooling

- CLI surfaces: `aivi check`, `compile`, `build`, `run`, `execute`, `fmt`, `lex`, `lsp`, `db migrate`, `db apply`.
