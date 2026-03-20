---
apply: always
---

# AIVI Language Reference — Token-Optimized LLM Context

Use this as the compact default reference for generating AIVI. If a case is unclear, the full spec/reference wins.

## 1. Core rules

- Statically typed, purely functional, expression-oriented.
- Immutable bindings only. No mutation, no loops, no null, no semicolons.
- Prefer `Option A`, `Result E A`, `Validation E A`.
- Bindings use `=` only. No `let`, `var`, `const`.
- Records are structural and closed.
- Effects are explicit: `Effect E A`.
- Functions are curried; application is by whitespace.
- Pattern bindings with `=` must be total; use `match` for refutable cases.
- Opening `{` stays on the same line for record/sigil forms: `patch { ... }`, `x match`, `~<gtk>...</gtk>`.
- Avoid deep nesting; extract helpers.

## 2. Lexical / naming

- Comments: `// ...`, `/* ... */` (non-nesting).
- lowerCamelCase: values/functions/fields.
- UpperCamelCase: types/constructors/domains/classes.
- snake_case: module path segments / file names.
- Text: `"hello { name }"`.
- Numbers: `42`, `3.14`, suffixed literals like `10px`, `30s`, `100%`.
- Constructors, not keywords: `True False None Some Ok Err`.
- Whole-value pattern binding uses `name as pat`; do not write `name@pat`.

## 3. Bindings / destructuring

```aivi
x = 42
add = a b => a + b
{name, age} = user
(a, b) = pair
[h, ...t] = list          // must be total
user as { name } = getUser
```

- Shadowing creates a new binding; it is not mutation.
- Top-level bindings are recursive.

Record pattern forms:

- `{ field }` bind field directly.
- `{ field: pat }` rename/match.
- `{ field as { pat } }` keep whole field and destructure.
- `{ field.{ pat } }` destructure only.
- Deep path destructuring allowed: `{ data.user.profile.name }`.

## 4. Functions / application

```aivi
add : Int -> Int -> Int
add = a b => a + b
inc = add 1
result = add 5 10
f = x => x + 1
g = _ + 1                 // unary placeholder lambda only
h = x <| _ + 1 => x * 2   // patch argument before body
```

- Application uses spaces, not parentheses: `f x y`.
- `.field` means accessor lambda: `x => x.field`.
- `x <| updater => body` is lambda-head sugar for “shadow `x` with `x |> updater`, then evaluate `body`”.
- The updater in that head form uses the same syntax as a pipe RHS (`_ + 1`, `.field`, explicit lambda, matcher block, ...).
- Multi-arg patched heads are typically formatted as:

  ```aivi
  f = x <| _ + 1
      y <| _ + 3 => ...
  ```

- Pipes pass the left value as the last argument:
    - `x |> f` == `f x`
    - `x |> f a b` == `f a b x`
- For `Signal A`, use `->>` for derivation.

Multi-clause unary functions require a type signature:

```aivi
describe : Int -> Text
describe =
  | 0 => "zero"
  | 1 => "one"
  | _ => "many"
```

For multi-arg matching, match a tuple.

## 5. `match`

```aivi
value match
  | Ok x  => x
  | Err _ => 0
```

- `match` consumes the expression immediately to its left.
- Must be exhaustive; use `_` as catch-all.
- Guards use `when` or `unless`.
- Whole-value binding works inside patterns with `as`.

## 6. Types

Primitives / common carriers:

- `Unit Bool Int Float`
- `Text Bytes Decimal BigInt`
- `DateTime Timestamp Date TimeZone ZonedDateTime`

ADTs:

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
Validation E A = Valid A | Invalid E
Color = Red | Green | Blue
Tree A = Node A (List (Tree A))
```

Branded nominal type:

```aivi
Email = Text!
```

Opaque type hides representation outside its module:

```aivi
opaque Url = { protocol: Text, host: Text, ... }
```

Closed records:

```aivi
User = { id: Int, name: Text, email: Option Text }
UserWithEmail = { ...User, email: Text }
```

Values:

```aivi
pair = (1, "hello")
xs = [1, 2, 3]
ys = [0, ...xs, 4]
zs = [1 .. 10]
```

Type-level row transforms:

```aivi
Pick (id, name) User
Omit (isAdmin) User
Optional (email) User
Required (email) User
Rename { createdAt: created_at } User
Defaulted (email) User
User |> Pick (id, name) |> Optional (name)
```

## 7. Classes / instances

Supported style:

```aivi
class Semigroup A = { concat: A -> A -> A }
class Monoid A = Semigroup { empty: A }
class Functor (F A) = { map: (A -> B) -> F B }
instance Monad (Option A) = { ... }
```

Key notes:

- Type variables are implicitly universally quantified.
- Use `given (A: ClassName)` only for real constraints.
- `use aivi.logic` brings class methods into scope.
- Class methods work both direct and piped: `map f xs` and `xs |> map f`.
- Zero-arg members like `empty` resolve from expected type; qualify if ambiguous.

## 8. Coercions

When expected type requires it:

- `Text` context may insert `toText` if `ToText` exists.
- `Body` / `ResponseBody` context can coerce:
    - record literal → `Json (toJson record)`
    - `Text` → `Plain text`
    - `JsonValue` → `Json ...`
    - `List Int` → `RawBytes ...` when `ResponseBody` is expected
- `Option A` context may wrap a plain `A` as `Some A`.

## 9. Predicates

Predicate expressions auto-lift to functions where needed:

```aivi
users |> filter active
users |> filter (age > 18)
users |> find (by id)
users |> find (email == Some "x")
xs |> takeWhile (_ < 10)
```

Rules:

- bare unbound name → field accessor
- `by prop` → `x => x.prop == prop`
- `by (f1, f2)` compares multiple fields
- `!`, `&&`, `||` compose predicates
- use pattern predicates for `Option` / `Result`
- `_.field` is invalid; use `.field` or bare field name

## 10. Patching

`<|` applies structural updates to a record.

```aivi
user2 = user <| { name: "Sam" }
user3 = user <| { profile.avatar: "new.png" }
record <| { items[*].price: _ * 1.1 }
record <| { items[price > 80].tag: "hot" }
record <| { lookup["key"]: newVal }
db.rows users[active]
db.first users[id == userId]
db.rows (
  users[active]
    |> orderBy (desc .createdAt, asc .id)
    |> limit 10
    |> selectMap { id: .id, email: .email }
)
db.update users[id == userId] { role: "admin" }
users[id == userId] <| { role: "admin" }
db.delete users[id == userId]
```

Patch instructions:

- `value` replace/insert
- `function` transform existing value
- `:= function` store function as data
- `-` remove field

Patch value:

```aivi
p = patch { name: toUpper }
result = user <| p
```

For `Signal A`, use `<<-`:

- plain value → `set`
- function → `update`
- record literal preserves patch semantics

## 11. Flows

Flat flow syntax is the default way to write sequential, fallible, applicative, zero-many, and handler-shaped workflows.

### Ordinary sequential flow

```aivi
loadConfig : Path -> Effect ConfigError Config
loadConfig = path =>
  file.json {
    path: path
    schema: source.schema.derive
  }
  |> load #cfg
 >|> cfg.enabled or fail DisabledConfig
  |> normalizeConfig
```

### Flow operators

- ` |>` — ordinary sequential step
- `~|>` — tap; run an effect and keep the incoming subject
- `>|>` — guard; use `or fail ...` for preconditions
- `?|>` / `!|>` — attempt and recover
- `||>` — branch over the current subject
- `*|>` — start a fan-out body over an iterable
- `*-|` — end the current fan-out body and rejoin the outer spine
- `&|>` — independent siblings over shared input; carrier type decides how they combine
- `@|>` — anchor for `recurse`

Bindings and modifiers:

- `#name` binds a successful line result
- modifiers: `@timeout`, `@delay`, `@concurrent`, `@retry`, `@cleanup`

### Applicative and zero-many shapes

```aivi
draft
   &|> validateTitle #title
   &|> validateEmail #email
    |> SaveRequest { title, email }

users
   *|> _
   >|> active
    |> .name
   *-|
```

## 12. Effects

```aivi
pure    : A -> Effect E A
fail    : E -> Effect E A
bind    : Effect E A -> (A -> Effect E B) -> Effect E B
attempt : Effect E A -> Effect F (Result E A)
```

Use flat flows for everyday effectful code.

```aivi
readText = path =>
  path
     |> file.read
     |> attempt
     |> result => result match
          | Ok text => text
          | Err _   => "(missing)"
```

- `attempt` captures effect errors as `Result` data.
- Use `>|>` for preconditions and `?|>` / `!|>` for inline recovery.
- `match` remains the ordinary branching tool for `Option`, `Result`, and ADTs.

## 13. Cleanup

```aivi
readAllText = path =>
  path
     |> file.open @cleanup file.close #handle
     |> file.readAll handle
```

- register cleanup with `@cleanup`
- cleanup runs on scope exit, error, or cancellation
- multiple cleanups unwind in LIFO order

## 14. Modules / imports / exports


```aivi
module my.app.api
export fetchUser, User
use aivi.net.http (get)
use aivi.json (decode)
```

Import forms:

```aivi
use aivi.text
use aivi.text (toUpper, toLower)
use aivi.text hiding (trim)
use aivi.text as T
use aivi.chronos.duration (domain Duration)
```

- One module per file.
- `module` is first non-empty item.
- Prelude is implicit via `use aivi.prelude`; disable with `@no_prelude`.
- Application module/file path segments should be `snake_case`.

## 15. External sources

Preferred: schema-first declarations, `load` inside flat flows.

- On GNOME desktops, `aivi.gnome.onlineAccounts` can resolve desktop-managed mail auth/config for use with `aivi.email`.

```aivi
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive
  }

cfgSource =
  env.decode {
    prefix: "AIVI_APP",
    schema: source.schema.derive
  }
```

Helpers in verified subset:

- `source.transform`
- `source.validate`
- `source.decodeErrors`

Also supports `@static` embedding and `openapi.fromUrl` / `openapi.fromFile` factories.
`@static type.jsonSchema TypeName` generates an OpenAI-compatible JSON Schema.

## 16. Sigils

Common sigils:

```aivi
~u(https://example.com)
~path[/usr/local/bin]
~r/[a-z]+/i
~map{ "a" => 1 }
~set[1, 2, 3]
~mat[1.0 0.0
     0.0 1.0]
~d(2024-05-21)
~dt(2024-05-21T12:00:00Z)
~tz(Europe/Paris)
~zdt(2024-05-21T12:00:00[Europe/Paris])
~k"app.button.save"
~m"Hello, {name}!"
~`raw text`
~<html><div>{ "x" }</div></html>
~<gtk><GtkBox spacing="24" /></gtk>
```

- `~map`, `~set`, `~mat`, `~<html>`, `~<gtk>` are structured.
- Other sigils are raw text until closing delimiter.
- Raw-text backtick sigil has no interpolation and supports multiline margin stripping.

GTK / reactive essentials:

- `signal { ... }` creates state.
- `signal ->> ...` derives signals.
- `set`, `update`, `<<-` mutate signal values declaratively.
- `event.from (...)` creates event handles from effects with shared lifecycle state.
- GTK sigils support shorthand `Gtk*` / `Adw*` tags, payload-oriented signal sugar (`onClick`, `onActivate`, `onInput`, `onToggle`, `onValueChanged`, `onSelect`, `onFocusIn`, `onFocusOut`, `onClosed`, `onShowSidebarChanged`) with direct callback payloads, raw `<signal ... />` escape hatches, `<each>`, `<show>`, component tags, function-call tags (including zero-arg `<Pane />` -> `pane Unit` sugar for simple self-closing helpers), and nested `<property name="..."> <Gtk.../> </property>` helper-object graphs for GTK object-valued properties such as `model` and `factory`.

## 17. Decorators

Built-in only:

- `@test "desc"`
- `@static`
- `@native "mod.fn"`
- `@native "crate::path::fn"`
- `@deprecated`
- `@debug` / `@debug(...)`
- `@no_prelude`

Unknown decorators are compile errors.

## 18. Mocking / testing

```aivi
mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
in fetchUsers
   |> users => assertEq (length users) 1
```

Rules:

- mock only qualified paths
- mock must be type-correct
- scope is limited to `in ...`
- nested mocks shadow outer mocks
- deep calls inside body see the mock

Snapshot support:

- `mock snapshot path`
- `assertSnapshot "name" value`
- helpers: `assertOk`, `assertErr`, `assertSome`, `assertNone`

## 19. Domains / suffix literals / operators

Domains define operator semantics and suffix literals for non-`Int` carriers.

```aivi
use aivi.chronos.duration (domain Duration)
deadline = start + 10min
```

- suffix literal resolves through a domain template like `1min`
- `(x)kg` applies suffix to parenthesized expression
- if imported domains collide on the same suffix, disambiguate by imports or constructors
- domains define operator meaning; they do not add implicit casts

Operator resolution:

- domain-resolved on non-`Int`: `+ - * × / %`
- built-in for primitives/text comparisons: `< <= > >=`
- always built-in: `== != && || |> <| ..`
- `×` is preferred for structural products (e.g. matrix × vector)

## 20. Precedence (low → high)

1. `|>`
2. `??`
3. `||`
4. `&&`
5. `== !=`
6. `< <= > >=`
7. `+ - ++`
8. `* × / %`
9. `<|`

Unary: `!`, unary `-`.
`~` is not a unary operator; it only starts sigils.
No bitwise operators in core syntax; use `aivi.bits`.

## 21. REPL

```bash
aivi repl
aivi repl --plain
```

- prelude loaded by default
- top-level effects autorun unless `/autorun off`
- useful commands: `/help`, `/explain`, `/use`, `/types`, `/values`, `/functions`, `/modules`, `/history`, `/load`, `/clear`

## 22. High-value idioms

```aivi
xs |> map f
xs |> filter (age > 18)
xs |> find (name == "Alice")
opt match | Some x => x | None => d
res match | Ok x => x | Err e => handle e
opt ?? default
opt |> getOrElse default
res |> getOrElse default
opt |> map f |> filter pred |> chain g
res |> map f |> mapErr g |> chain h
Unit
  &|> v1 #x
  &|> v2 #y
   |> f x y
request
  ?|> risky
  !|> Timeout _ => default
input
  >|> isValid or fail err
state <| { user.profile.name: "New" }
state <| { items[*].price: _ * 1.1 }
src
  *|> _
  >|> pred
   |> f
  *-|
```

## 23. Anti-patterns (never emit)

- No `let`, `var`, `const`, `fn`, `def`, `return`, `import`.
- No semicolons.
- No mutation.
- No `null`, `nil`, `Just`, `Nothing`, `Left`, `Right`.
- No `Some(x)` / `Ok(x)` constructor-paren style.
- No loop syntax: `for`, `while`.
- No paren-call style: write `f x`, not `f(x)`.
- No method-call style: use `map f xs` or `xs |> map f`.
- No angle-bracket generics: `List Int`, not `List<Int>`.
- No `flatMap`; use `chain`.
- No `generate { ... }`, `Generator`, or `aivi.generator`; use lists, ranges, collection helpers, or `*|>` ... `*-|` fan-out.
- No `case ... of`; use `match`.
- No `String`; use `Text`.
- Record fields use `:`, not `=`.
- No bitwise infix operators; use `aivi.bits` functions.
- No `~` as bitwise-not.
- Prefer interpolation over string concat operators.
- Keep opening `{` on same line as keyword.
- do not chain if/else statements, prefer pattern matching
- using ADT.func if func is not ambigeous. Ie: Prefer isSome to Option.isSome
- do not nest "match" constructs

## 24. Minimal generation checklist

When writing AIVI, default to:

1. `module ...`
2. `use ...`
3. type aliases / ADTs / opaque / branded types
4. signatures for public functions and all multi-clause functions
5. curried defs with `=` and `=>`
6. `match` for refutable branching
7. flow syntax (`|>`, `>|>`, `?|>`, `!|>`, `&|>`, `*|>` ... `*-|`) for workflows
8. `Option` / `Result` instead of null / exceptions
9. pipelines, predicates, patches, recursion, and fan-out flows
10. concise helpers instead of nested expressions
