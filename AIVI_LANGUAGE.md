---
apply: always
---

# AIVI Language Reference (LLM Context)

> This file is the authoritative quick-reference for writing AIVI code.
> For the full specification see `specs/`. When in doubt, the specs win.

---

## 1 Core Design

- Statically typed, purely functional, expression-oriented.
- Immutable bindings; **no mutation**, no loops, no null.
- Use `Option A` / `Result E A` instead of null; recursion, folds, or generators instead of loops.
- Pattern bindings with `=` must be **total**; refutable matches use `match`.
- Records are structural and open (row polymorphism).
- Effects are explicit: `Effect E A` (error type `E`, success type `A`).
- Domains give meaning to operators and suffix literals for non-`Int` types.

---

## 2 Lexical Basics

| Element                                            | Syntax                                                                                                                                                         |
|:-------------------------------------------------- |:-------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Line comment                                       | `//` to end of line                                                                                                                                            |
| Block comment                                      | `/* ... */` — may span multiple lines; **does not nest**                                                                                                       |
| Value / function / field names                     | `lowerCamelCase` (`lowerIdent`)                                                                                                                                |
| Type / constructor / domain / class names          | `UpperCamelCase` (`UpperIdent`)                                                                                                                                |
| Module path segments / `.aivi` file names          | `snake_case` (for example `myapp.daemon.command_queue` -> `myapp/daemon/command_queue.aivi`)                                                             |
| Text literal                                       | `"hello { name }"` (interpolation with `{ expr }`)                                                                                                             |
| Int, Float                                         | `42`, `3.14`                                                                                                                                                   |
| Char                                               | `'a'`                                                                                                                                                          |
| ISO instant                                        | `2024-05-21T12:00:00Z`                                                                                                                                         |
| Suffixed number                                    | `10px`, `30s`, `100%` (domain-resolved)                                                                                                                        |
| Keywords                                           | `as class do domain effect else export generate given hiding if instance machine match module on or over patch recurse resource then use when with yield loop` |

`True`, `False`, `None`, `Some`, `Ok`, `Err` are constructors, not keywords.

---

## 3 Bindings and Scope

All bindings use `=`. There is no `let`, `var`, or `const`.

```aivi
x = 42
add = a b => a + b
```

### Destructuring

```aivi
{ name, age } = user             // record destructuring
(a, b) = pair                    // tuple destructuring
[h, ...t] = list                 // list head/tail (must be total)
user as { name } = getUser       // whole-value + destructure
```

### Record pattern operators

| Syntax                 | `field` in scope? | Inner bindings? | Purpose                         |
|:---------------------- |:-----------------:|:---------------:|:------------------------------- |
| `{ field: pat }`       | no (renamed)      | yes             | Match / rename a field          |
| `{ field as { pat } }` | yes               | yes             | Keep whole field + destructure  |
| `{ field.{ pat } }`    | no                | yes             | Destructure only, discard field |
| `{ field }`            | yes               | -               | Shorthand, binds field by name  |

Deep path destructuring: `{ data.user.profile.name }` reaches nested fields directly.

### Shadowing

Shadowing is allowed. It introduces a new binding; there is no mutation.

```aivi
x = 1
x = x + 1  // new binding, old `x` is no longer accessible
```

### Recursion

Top-level module bindings are recursive (can refer to themselves and later bindings). Local recursion uses module-level helpers or generators with `loop`/`recurse`.

---

## 4 Functions

### Definition and application

Functions are **curried**. Application is by whitespace.

```aivi
add : Int -> Int -> Int
add = a b => a + b

result = add 5 10       // 15
inc = add 1             // partial application
```

### Lambdas

```aivi
x => x + 1              // explicit lambda
_ + 1                   // placeholder lambda (unary only)
a b => a + b            // multi-argument
```

`_` in expression position is a single-argument placeholder lambda. It is only valid where a unary function is expected.

### Pipes (`|>`)

Pipes apply the value on the left as the **last** argument to the right-hand side.

```aivi
xs |> map inc |> filter (_ > 0)

// equivalent to:
filter (_ > 0) (map inc xs)
```

Rules: `x |> f` = `f x`; `x |> f a b` = `f a b x`.

### Multi-clause functions

A unary function can be written as multiple match arms directly:

```aivi
describe =
  | 0 => "zero"
  | 1 => "one"
  | _ => "many"
```

Multi-clause definitions require an explicit type signature for that function name.
With closed records, this keeps each clause checked against the exact input record shape.
If no clause matches at runtime, evaluation fails with a non-exhaustive-match runtime error.

For multi-argument matching, match on a tuple:

```aivi
gcd = (a, b) => (a, b) match
  | (x, 0) => x
  | (x, y) => gcd y (x % y)
```

### Accessor sugar

`.field` (with dot prefix) is shorthand for `x => x.field`:

```aivi
users |> map .name // or map _.name or map name
```

---

## 5 Pattern Matching (`match`)

`match` takes the expression **immediately to its left** and tests it against arms.

```aivi
value match
  | Ok x  => x
  | Err _ => 0
```

Works with pipelines:

```aivi
input |> parse |> validate match
  | Ok x  => x
  | Err e => handle e
```

### Guards

```aivi
classify = n => n match
  | _ when n > 0 => "positive"
  | _ when n < 0 => "negative"
  | _            => "zero"
```

### Whole-value binding in patterns

```aivi
response match
  | { data.user.profile as { name } } => name
  | { data.guest: True }               => "Guest"
  | _                                  => "Unknown"
```

### Nested constructor patterns

```aivi
parse = expr => expr match
  | Add (Lit a) (Lit b) => Lit (a + b)
  | _                   => expr
```

### Exhaustiveness

All `match` expressions must be exhaustive. Non-exhaustive matches are compile errors. Use `_` as a catch-all.

---

## 6 Types

### Compiler primitives

```aivi
Unit  Bool  Int  Float
```

### Standard library types

```aivi
Text  Bytes  Decimal  BigInt
Duration  Instant  Date  Time  TimeZone  ZonedDateTime
```

### Algebraic data types (ADTs)

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
Color = Red | Green | Blue
Tree A = Leaf A | Node (Tree A) (Tree A)
```

Create values by applying constructors: `Some 42`, `Err "nope"`, `Node (Leaf 1) (Leaf 2)`.
Nullary constructors (`None`, `True`, `Red`) are values directly.
Use `constructorName value` and `constructorOrdinal value` to inspect an ADT value at runtime.
`constructorOrdinal` is zero-based by constructor declaration order.

### Branded nominal types

```aivi
Email = Text!

mkEmail : Text -> Email
mkEmail = text => Email text
```

`T = U!` creates a nominal type `T` (distinct from `U`) using a single constructor named `T`.
Instances for the base type are auto-forwarded to the branded type when available; explicit
instances for the branded type take precedence.

### Open records (row polymorphism)

```aivi
User = { id: Int, name: Text, email: Option Text }

// Functions require minimum fields, accept more:
greet : { name: Text } -> Text
greet = user => "Hello, { user.name }"
```

### Record creation and spread

```aivi
p = { x: 1, y: 2 }
q = { ...p, x: 3 }          // later fields win: { x: 3, y: 2 }
```

### Tuples and lists

```aivi
pair = (1, "hello")
xs = [1, 2, 3]
ys = [0, ...xs, 4]          // spread
zs = [1 .. 10]              // range (inclusive)
```

### Row transforms (type-level)

```aivi
Pick (id, name) User
Omit (isAdmin) User
Optional (email) User
Required (email) User
Rename { createdAt: created_at } User

// Type-level pipe:
User |> Pick (id, name) |> Optional (name)
```

### Type signatures

```aivi
add : Int -> Int -> Int
map : (A -> B) -> List A -> List B
```

Type signatures are also required for multi-clause function definitions (`f = | ... => ...`).

### Classes and instances (ad-hoc polymorphism, HKTs)

```aivi
class Functor (F *) = {
  map : (A -> B) -> F A -> F B
}

class Apply (F *) = Functor {
  ap : F A -> F (A -> B) -> F B
}

instance Monad (Option *) = { ... }
instance Monad (Result E *) = { ... }
```

`*` denotes a higher-kinded type (F takes one type argument).
`A with B` in types is record/type composition (intersection).

### Type variable constraints

```aivi
class Collection (C *) = given (A: Eq) {
  elem : A -> C A -> Bool
  unique : C A -> C A
}
```

### Expected-type coercions

In positions where a `Text` is expected, the compiler may insert `toText expr` if a `ToText A` instance is in scope. This is the only coercion mechanism (no global implicit casts).

---

## 7 Predicates

Predicate expressions are `Bool` expressions that can be used directly where a function `A -> Bool` is expected (auto-lifted).

```aivi
users |> filter active               // active is _.active
users |> filter (age > 18)           // age is _.age
users |> find (email == Some "x")
xs |> takeWhile (_ < 10)
```

Inside predicates:

- `_` is the current element.
- Bare field names resolve to `_.field`.
- `.field` is an accessor function, not a field value.
- Combinators: `!p`, `p && q`, `p || q`.
- Pattern predicates: `Some _`, `Ok { value } when value > 10`.

---

## 8 Patching (Structural Updates)

`<|` applies a declarative, type-checked patch to a record.

```aivi
user2 = user <| { name: "Sam" }
user3 = user <| { profile.avatar: "new.png" }
```

### Path addressing

```aivi
record <| { a.b.c: value }                // dot paths
record <| { items[*].price: _ * 1.1 }     // traversal (all items)
record <| { items[price > 80].tag: "hot" } // predicate selector
record <| { lookup["key"]: newVal }        // map key selector
```

### Instructions

| Instruction   | Meaning                                         |
|:------------- |:----------------------------------------------- |
| `value`       | Replace or insert                               |
| `function`    | Transform existing value (applied to old value) |
| `:= function` | Replace with function **as data** (not applied) |
| `-`           | Remove field (shrinks record type)              |

### Patch-as-value

```aivi
p = patch { name: toUpper }     // Patch User, i.e. User -> User
result = user <| p
```

---

## 9 Blocks

AIVI has four block forms, each introduced by a keyword and delimited with `{ ... }`.

### `generate { ... }` - Pure sequences

Pull-based, lazy sequences. No effects, no suspension.

```aivi
gen = generate {
  yield 1
  yield 2
  yield 3
}
```

**Bindings, guards, transforms:**

```aivi
evens = generate {
  x <- [1 .. 100]
  x -> x % 2 == 0           // guard (filter)
  yield x
}
```

**Cartesian product:**

```aivi
pairs = generate {
  x <- [1 .. 3]
  y <- ["a", "b"]
  yield (x, y)
}
```

**Tail-recursive loops:**

```aivi
fibs = generate {
  loop (a, b) = (0, 1) => {
    yield a
    recurse (b, a + b)
  }
}
```

Summary of statements inside `generate { ... }`:

- `x <- source` - bind from another generator/list
- `x = expr` - pure local binding
- `x -> pred` - guard (filter by predicate)
- `yield expr` - emit a value
- `loop pat = init => { ... recurse next }` - local tail recursion

### `do Effect { ... }` - Effectful computation

The most common block form. Sequences effectful operations with typed errors.

```aivi
main = do Effect {
  cfg  <- load (file.read "config.json")   // <- runs effect
  name = cfg.appName                        // = is pure binding
  print "loaded { name }"
}
```

Statements inside `do Effect { ... }`:

- `x <- eff` - run effect, bind result
- `_ <- eff` - run effect and intentionally discard result (optional; use when value is unused)
- `x = expr` - pure local binding (`expr` must NOT be `Effect`)
- `x <- resource` - acquire a `Resource`, released on scope exit
- `when cond <- eff` - conditional (runs `eff` only when `cond` is true)
- `given cond or failExpr` - precondition guard
- Final expression must be `Effect E A` (commonly `pure value`)
- Statement expressions must be `Effect E Unit`; non-Unit results must be bound

**Tail-recursive loops in effect blocks:**

`loop`/`recurse` works inside `do Effect { ... }` blocks for stateful iteration:

```aivi
dijkstra = source graph => do Effect {
  dists0 = Map.insert source 0.0 Map.empty

  loop state = { dists: dists0, pq: Heap.push (0.0, source) Heap.empty } => {
    dists = state.dists
    pq = state.pq
    result = Heap.popMin pq
    result match
      | None                       => pure dists
      | Some ((d, node), restPq)   => do Effect {
          currentDist = Map.getOrElse node 999999.0 dists
          if d > currentDist
          then do Effect { recurse { dists: dists, pq: restPq } }
          else do Effect {
            edges = edgesFrom graph node
            nextState <- processEdges dists d edges restPq
            recurse nextState
          }
        }
  }
}
```

- `loop pat = init => { body }` - local tail-recursive loop (same syntax as in generators)
- `recurse newState` - continue with the next iteration
- Omitting `recurse` in a branch terminates the loop
- The loop body `{ ... }` is promoted to the parent effect-block kind, so `<-`, `when`/`unless`, and `recurse` work inside

**Event wiring (`on`):**

Inside `do Effect { ... }`, `on Event => handler` wires event handlers for state machine transitions or UI events:

```aivi
main = do Effect {
  on Click => do Effect {
    count <- getState
    setState (count + 1)
  }
  on KeyPress => handleKey
}
```

### `do M { ... }` - General monadic blocks

`do Monad { ... }` is the general form; `do Effect { ... }` is the most common specialisation. `Option` and `Result` are also supported:

```aivi
// do Option: short-circuits on None
safeLookup = key1 key2 map => do Option {
  x <- Map.get key1 map
  y <- Map.get key2 map
  Some (x + y)
}

// do Result: short-circuits on Err
validateAge = input => do Result {
  n <- parseInt input
  ok <- if n >= 0 && n <= 150 then Ok n else Err "out of range"
  Ok ok
}
```

The same bind (`<-`) and pure-bind (`=`) syntax applies. Statement availability by block kind:

| Statement                   | `do Effect` | `do M` (generic) | `generate`        |
|:--------------------------- |:-----------:|:----------------:|:-----------------:|
| `x <- expr`                 | ✓           | ✓                | ✓ (from sequence) |
| `x = expr`                  | ✓           | ✓                | ✓                 |
| `expr` (sequencing)         | ✓           | ✓                | —                 |
| `yield expr`                | —           | —                | ✓                 |
| `x -> pred` (guard)         | —           | —                | ✓                 |
| `or` fallback               | ✓           | —                | —                 |
| `when`/`unless cond <- eff` | ✓           | —                | —                 |
| `given cond or expr`        | ✓           | —                | —                 |
| `on Event => handler`       | ✓           | —                | —                 |
| `loop`/`recurse`            | ✓           | —                | ✓                 |
| resource `<-`               | ✓           | —                | —                 |

Effect-specific statements (`or`, `when`, `unless`, `given`, `on`, resource `<-`, `loop`/`recurse`) are **only** available in `do Effect` blocks.

### `machine { ... }` - State machines

`machine` declares a state machine where transitions are first-class and states are inferred.

```aivi
machine Door = {
           -> Closed : init   {}
  Closed   -> Opened : open   {}
  Opened   -> Closed : close  {}
  Opened   -> Locked : lock   {}
  Locked   -> Closed : unlock {}
}
```

`-> State : init {}` marks the starting state. `Source -> Target : event { payload }` defines transitions with optional typed payloads. States are inferred from the transition graph. The compiler checks completeness and type safety.

Runtime machine values are available by machine name and can be destructured as records:

```aivi
do Effect {
  { lease, run, done, currentState, can } = AccountSyncMachine
  _ <- assertEq (constructorName (currentState Unit)) "Idle"
  _ <- lease {}
  _ <- run { batchId: 42 }
  _ <- done {}
}
```

`can.<transition> Unit` reports if the transition is valid from the current state.
Invalid transition calls fail with `InvalidTransition { machine, from, event, expectedFrom }`.
`on transition => handler` handlers run after a successful state transition applies.

---

## 10 Effects

`Effect E A` models typed effects where `E` is the error type and `A` is the success type.

### Core operations

| Operation | Type                                            | Purpose                 |
|:--------- |:----------------------------------------------- |:----------------------- |
| `pure`    | `A -> Effect E A`                               | Lift a value            |
| `fail`    | `E -> Effect E A`                               | Abort with error        |
| `bind`    | `Effect E A -> (A -> Effect E B) -> Effect E B` | Sequence                |
| `attempt` | `Effect E A -> Effect F (Result E A)`           | Catch error as `Result` |

### Error fallback with `or`

```aivi
// Effect fallback (inside do Effect block after <-):
txt <- load (file.read path) or "(missing)"

// With error pattern matching:
val <- riskyOp or
  | NotFound msg => pure default
  | Timeout _    => fail "timed out"

// Result fallback (expression form):
count = result or 0
```

`or` arms match the **error value** directly (write `NotFound m`, not `Err NotFound m`).

### Attempt (error recovery)

```aivi
getUser = id => do Effect {
  res <- attempt (api.fetchUser id)
  res match
    | Ok user => pure user
    | Err _   => pure GuestUser
}
```

`attempt` catches errors of type `E`, producing `Result E A`. The outer effect has error type `F` (different from `E`).

### Conditional effects (`when`)

`when cond <- eff` runs `eff` only when `cond` is true. Otherwise the block continues with `Unit`.

```aivi
main = do Effect {
  cfg <- loadConfig
  when cfg.verbose <- print "verbose mode enabled"
  when cfg.dryRun <- print "dry run, no side effects"
  process cfg
}
```

Desugars to: `_ <- if cond then eff else pure Unit`.

### Precondition guards (`given`)

`given cond or failExpr` asserts a condition. If `cond` is false, `failExpr` is evaluated (typically `fail`).

```aivi
withdraw = amount account => do Effect {
  given amount > 0 or fail (InvalidAmount amount)
  given account.balance >= amount or fail InsufficientFunds
  updateBalance account (account.balance - amount)
}
```

`given` also supports match arms on the condition's value:

```aivi
main = do Effect {
  given validate input or
    | InvalidField f => fail (BadRequest f)
    | MissingField f => fail (BadRequest "missing: { f }")
  process input
}
```

Desugars to: `_ <- if cond then pure Unit else failExpr`.

### Branching in effect blocks

`if`/`then`/`else` is an expression. For multi-step branches, use nested `do Effect { ... }`:

```aivi
process = input => do Effect {
  validated <- validate input
  result <- if validated.needsReview then
    do Effect {
      _ <- notifyReviewer validated
      pure (Pending validated)
    }
  else pure (Approved validated)
  pure result
}
```

---

## 11 Resources

`Resource E A` - a recipe for acquiring a handle of type `A` (with error type `E`), using it, and releasing it.

### Defining

```aivi
managedFile = path => resource {
  handle <- file.open path       // acquire
  yield handle                   // provide to caller
  file.close handle              // release (runs on scope exit)
}
```

Rules: exactly one `yield`; code after `yield` is cleanup; cleanup may perform effects.

### Using

```aivi
main = do Effect {
  f <- managedFile "data.txt"       // acquired here
  content <- file.readAll f
  print content
}                                    // f released here (LIFO)
```

Multiple resources are released in reverse acquisition order. Cleanup runs even on error or cancellation.

---

## 12 Modules and Imports

One module per file. `module` must be the first non-empty item.

```aivi
module my.app.api
export fetchUser, User

use aivi.net.http (get)
use aivi.json (decode)

User = { id: Int, name: Text }

fetchUser : Int -> Effect HttpError User
fetchUser = id => do Effect {
  resp <- get (~u(https://api.example.com/users/{ id }))
  decode resp.body
}
```

### Import forms

```aivi
use aivi.text                            // import all public symbols
use aivi.text (toUpper, toLower)         // selective
use aivi.text hiding (trim)             // import all except
use aivi.text as T                       // aliased module
use aivi.chronos.duration (domain Duration)  // import domain
```

### Export forms

```aivi
export add, subtract, pi                 // selective
export domain Color                      // export domain
```

### Prelude

Every module implicitly does `use aivi.prelude`. Disable with `@no_prelude`.

### Module path convention

- `aivi.*` - standard library
- `aivi.chronos.*` - time/date/duration/timezone
- `aivi.net.*` - networking (http, https, rest, httpServer)
- `aivi.rest` - REST-oriented source facade
- `aivi.email` - IMAP email source facade
- `aivi.list` - List operations (map, filter, fold, ...)
- `aivi.map` - Map (ordered key-value)
- `aivi.set` - Set (ordered unique elements)
- `aivi.queue` - Queue / Deque
- `aivi.heap` - Min/Max heap
- `vendor.name.*` - third-party libraries
- `user.app.*` - application code
- In application code, module path segments and backing file names should be `snake_case`.

---

## 13 External Sources

`Source K A` represents typed external data. Load with `load` inside `do Effect { ... }`.

```aivi
cfg <- load (file.read "config.json")
cfgTyped <- load (file.json "config.json")
rows <- load (file.csv "users.csv")
resp <- load (rest.get ~u(https://api.example.com/data))
apiKey <- load (env.get "API_KEY")
appCfg <- load (env.decode "AIVI_APP")
```

Available source APIs in v0.1: `file.read/json/csv/imageMeta/image`, `http`/`https`, `rest`, `env.get/decode`, `email.imap`.

`@static` embeds sources at compile time: `@static schema = file.json "schema.json"` or `@static envName = env.get "AIVI_BUILD_ENV"`.

---

## 14 Sigils

Custom literals with `~tag` and a delimiter:

```aivi
~u(https://example.com)        // URL
~path[/usr/local/bin]          // Path
~r/[a-z]+/i                    // Regex
~map{ "a" => 1, "b" => 2 }    // Map K V literal
~set[1, 2, 3]                  // Set A literal
~mat[1.0 0.0                   // Matrix literal (Mat2/Mat3/Mat4)
     0.0 1.0]
~d(2024-05-21)                 // Date
~t(12:00:00)                   // Time
~tz(Europe/Paris)              // TimeZone
~<html><div>{ "x" }</div></html> // Typed VDOM node
~<gtk><object class="GtkBox" /></gtk> // Typed GTK builder node
```

`~map{...}`, `~set[...]`, `~mat[...]`, `~<html>...</html>`, and `~<gtk>...</gtk>` are **structured** (parsed as AIVI expressions). Other sigils are raw text until the closing delimiter.

GTK sigils also support signal sugar in v0.1:

```aivi
~<gtk>
  <object class="GtkButton" onClick={ Msg.Save } />
  <object class="GtkEntry" onInput={ Msg.Changed } />
  <object class="GtkButton">
    <signal name="clicked" on={ Msg.Save } />
  </object>
</gtk>
```

Signal handlers must be compile-time expressions; they lower into typed GTK signal bindings.
`onClick` maps to `clicked`, `onInput` maps to `changed`, and invalid dynamic handlers produce `E1614`.

Dynamic child lists are supported with `<each>`:

```aivi
~<gtk>
  <object class="GtkBox">
    <each items={items} as={item}>
      <child><object class="GtkLabel"><property name="label">{ item }</property></object></child>
    </each>
  </object>
</gtk>
```

Component-style tags are also supported in both sigils:

```aivi
~<html><Ui.Card title="Hello"><span>Body</span></Ui.Card></html>
~<gtk><Ui.Row id="r1" /></gtk>
```

---

## 15 Decorators (v0.1)

Compile-time metadata only. No user-defined decorators.

| Decorator                                      | Purpose                                   |
|:---------------------------------------------- |:----------------------------------------- |
| `@test "desc"`                                 | Mark as test case (mandatory description) |
| `@static`                                      | Embed at compile time                     |
| `@native "mod.fn"`                             | Bind typed def to runtime/native path     |
| `@inline`                                      | Always inline                             |
| `@deprecated`                                  | Emit warning on use                       |
| `@debug` / `@debug(pipes, args, return, time)` | Debug tracing (with `--debug-trace`)      |
| `@no_prelude`                                  | Skip implicit `use aivi.prelude`          |

Unknown decorators are compile errors.
`@native` is only valid on top-level definitions and requires an explicit type signature.

---

## 16 Domains, Units, and Operators

Domains define operator semantics and suffix literals for non-`Int` types.

```aivi
use aivi.chronos.duration (domain Duration)

deadline = { millis: 0 } + 10min     // + resolved by Duration domain
```

### Suffix literals

Suffix literals are numeric literals followed immediately by a suffix identifier. They resolve to domain-defined **template functions** named `1{suffix}`:

```aivi
10min  30s  100px  50%  2w  3d
```

Suffix can also be applied to a parenthesized expression (variable suffix):

```aivi
(x)kg       // desugars to 1kg applied to x; parentheses required, no space before suffix
```

Common suffix → domain mapping:

| Suffix                     | Domain   | Type            |
|:-------------------------- |:-------- |:--------------- |
| `10ms`, `1s`, `5min`, `2h` | Duration | `Duration`      |
| `1d`, `2w`, `3mo`, `1y`    | Calendar | `CalendarDelta` |
| `20deg`, `1.2rad`          | Angle    | `Angle`         |
| `10l`, `5s`, `30h`         | Color    | `ColorDelta`    |

**Collision rule**: if two imported domains define the same suffix (e.g. both define `1m`), the compiler does not disambiguate by carrier. Resolve by importing only one conflicting domain per module, using `hiding`, or using explicit constructors instead.

### Defining domains

A domain declares one **carrier type** and may contain multiple operator entries for the same token (RHS-typed overloads), provided the full `LHS -> RHS -> Result` types are pairwise distinct. The compiler selects among them by matching the inferred RHS type after the LHS carrier is resolved.

```aivi
domain Color over Rgb = {
  Delta = Lightness Int | Hue Int

  (+) : Rgb -> Delta -> Rgb
  (+) = color (Lightness amount) => adjustLightness color amount
  (+) = color (Hue amount) => adjustHue color amount

  1l = Lightness 1     // suffix template: 3l desugars to Lightness 3
  1h = Hue 1
}
```

**`×` convention**: use `×` for structural/transform-style products (matrix × matrix, matrix × vector) and `*` for scalar scaling. This makes the visual intent explicit.

```aivi
domain LinAlg over Mat3 = {
  (×) : Mat3 -> Mat3 -> Mat3     // matrix-matrix product
  (*) : Mat3 -> Float -> Mat3   // scalar scaling
}
```

### Multi-carrier domains

For types that need the same domain semantics at different arities (e.g. `Vec2` and `Vec3`), define the domain once per carrier:

```aivi
domain Vector over Vec2 = { ... }
domain Vector over Vec3 = { ... }
```

### Import/export

```aivi
use aivi.chronos.duration (domain Duration)    // import domain
export domain Color                             // export domain
```

### Built-in vs domain-resolved operators

Domain-resolved (when non-`Int`): `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=`.
Always built-in: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`.

**Domains are not implicit casts.** They supply operator semantics and literal templates only. No global coercions are introduced by importing a domain.

---

## 17 Operator Precedence (lowest to highest)

1. `|>` (pipe)
2. `??` (coalesce Option)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `|` (bitwise or)
8. `^` (bitwise xor)
9. `<<`, `>>` (shift)
10. `+`, `-`, `++` (add, concat)
11. `*`, `×`, `/`, `%` (multiply)
12. `<|` (patch)

Unary prefix: `!` (not), `-` (negate), `~` (bitwise complement).

---

## 18 Complete Example

```aivi
@no_prelude
module integrationTests.complex.TopologicalSort

use aivi
use aivi.testing
use aivi.list
use aivi.map

Graph = { nodes: List Int, adj: Map Int (List Int) }

neighbors = node graph => Map.get node graph.adj match
  | Some ns => ns
  | None    => []

reverseList = xs => reverseGo xs []

reverseGo = list acc => list match
  | []        => acc
  | [h, ...t] => reverseGo t [h, ...acc]

buildIndegree : Graph -> Map Int Int
buildIndegree = graph => {
  start = initIndegree graph.nodes Map.empty
  processNodes graph graph.nodes start
}

initIndegree = nodes acc => nodes match
  | []        => acc
  | [n, ...t] => initIndegree t (Map.insert n 0 acc)

topologicalSort : Graph -> Result (List Int) (List Int)
topologicalSort = graph => {
  indeg = buildIndegree graph
  q0 = enqueueZero graph.nodes indeg Queue.empty
  sortLoop graph indeg q0 []
}

@test "Kahn topological sort"
topoSmoke = do Effect {
  adj = ~map{
    0 => [1, 2]
    1 => [3]
    2 => [3]
    3 => []
  }
  graph = { nodes: [0, 1, 2, 3], adj: adj }

  result <- pure (topologicalSort graph)
  assert (result == Ok [0, 1, 2, 3] || result == Ok [0, 2, 1, 3])
}
```

---

## 19 Quick Idiom Reference

| Task                         | AIVI idiom                                                              |
|:---------------------------- |:----------------------------------------------------------------------- |
| Transform a list             | `xs \|> map f`                                                          |
| Filter a list                | `xs \|> filter (age > 18)`                                              |
| Find first match             | `xs \|> find (name == "Alice")`                                         |
| Handle Option                | `opt match \| Some x => x \| None => default`                           |
| Handle Result                | `res match \| Ok x => x \| Err e => handle e`                           |
| Provide default for Option   | `opt ?? default`                                                        |
| Run fallback on effect error | `val <- riskyOp or default` (inside `do Effect`)                        |
| Catch error as Result        | `res <- attempt riskyOp` (inside `do Effect`)                           |
| Conditional effect           | `when cond <- eff` (inside `do Effect`)                                 |
| Precondition guard           | `given cond or failExpr` (inside `do Effect`)                           |
| Update nested record         | `state <\| { user.profile.name: "New" }`                                |
| Transform nested field       | `state <\| { items[*].price: _ * 1.1 }`                                 |
| Create map                   | `~map{ "key" => value }`                                                |
| Create set                   | `~set[1, 2, 3]`                                                         |
| Build a sequence             | `generate { x <- src; x -> pred; yield f x }`                           |
| Infinite sequence            | `generate { loop s = init => { yield s; recurse (next s) } }`           |
| State machine                | `machine Name = { -> Idle : init {}; Idle -> Running : start {}; ... }` |
| Acquire resource             | `handle <- managedFile "data.txt"` (inside `do Effect`)                 |
| Write a test                 | `@test "adds correctly" myTest = do Effect { assertEq (f 1) 2 }`        |

---

## 20 Anti-Patterns (Do NOT write these)

| Wrong                  | Why                              | Correct                                             |
|:---------------------- |:-------------------------------- |:--------------------------------------------------- |
| `let x = 1`            | No `let` keyword                 | `x = 1`                                             |
| `def f(x):`            | No `def`, no parens for args     | `f = x => ...`                                      |
| `var x = 1; x = 2`     | No mutation                      | `x = 1; x = x + 1` (shadow)                         |
| `null` / `nil`         | No nulls                         | `None` / `Option A`                                 |
| `throw` / `try/catch`  | No exceptions                    | `fail e` / `attempt` / `or`                         |
| `for x in xs { ... }`  | No loops                         | `xs \|> map f` or `generate { x <- xs; yield f x }` |
| `while cond { ... }`   | No loops                         | Recursion or `loop`/`recurse` in generators         |
| `x.method()`           | No methods, no parens            | `method x` or `x \|> method`                        |
| `case x of ...`        | `case` is kernel only            | `x match \| pat => expr`                            |
| `String`               | Type is called `Text`            | `Text`                                              |
| `return x`             | No return statement              | Expression result is implicit; `pure x` in effects  |
| `{ x = 1 }` in records | `=` is binding, not record field | `{ x: 1 }`                                          |
| `import X`             | No `import` keyword              | `use module.path`                                   |
