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
- Use `Option A` / `Result E A` instead of null; `Validation E A` for error accumulation; recursion, folds, or generators instead of loops.
- Pattern bindings with `=` must be **total**; refutable matches use `match`.
- Records are structural and closed (no row polymorphism).
- Effects are explicit: `Effect E A` (error type `E`, success type `A`).
- Domains give meaning to operators and suffix literals for non-`Int` types.
- No semicolons — bindings and block statements are separated by newlines.
- Opening `{` always on the same line as the keyword (`do Effect {`, `generate {`, `x match`).
- Avoid deeply nested expressions — extract inner logic into named helper functions.

---

## 2 Lexical Basics

| Element                                   | Syntax                                                                                                                                                                                 |
|:----------------------------------------- |:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Line comment                              | `//` to end of line                                                                                                                                                                    |
| Block comment                             | `/* ... */` — may span multiple lines; **does not nest**                                                                                                                               |
| Value / function / field names            | `lowerCamelCase` (`lowerIdent`)                                                                                                                                                        |
| Type / constructor / domain / class names | `UpperCamelCase` (`UpperIdent`)                                                                                                                                                        |
| Module path segments / `.aivi` file names | `snake_case` (for example `myapp.daemon.command_queue` -> `myapp/daemon/command_queue.aivi`)                                                                                           |
| Text literal                              | `"hello { name }"` (interpolation with `{ expr }`)                                                                                                                                     |
| Int, Float                                | `42`, `3.14`                                                                                                                                                                           |
| Char                                      | `'a'`                                                                                                                                                                                  |
| ISO date-time / timestamp carrier         | `2024-05-21T12:00:00Z`                                                                                                                                                                 |
| Suffixed number                           | `10px`, `30s`, `100%` (domain-resolved)                                                                                                                                                |
| Keywords                                  | `as class do domain effect else export generate given hiding if in instance machine match mock module on opaque or over patch recurse resource snapshot then unless use when with yield loop` |

`True`, `False`, `None`, `Some`, `Ok`, `Err` are constructors, not keywords.

`effect { }` is a **deprecated** alias for `do Effect { }` (emits warning W1600); always write `do Effect { }`.
`@` in patterns is **deprecated** in favour of `as` (emits warning W1603); write `name as pattern` instead of `name@pattern`.

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
users |> map name // or map _.name
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
  | { data.user.profile as { name } }  => name
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

### Standard library carriers and aliases

```aivi
Text  Bytes  Decimal  BigInt
DateTime  Timestamp  Date  TimeZone  ZonedDateTime
```

`Timestamp = DateTime` in `aivi.chronos.instant`. `Duration` and `Instant` are important library-facing names, but they are not separate primitive carriers in the current verified v0.1 surface.

### Algebraic data types (ADTs)

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
Validation E A = Valid A | Invalid E
Color = Red | Green | Blue
Tree A = Node A (List (Tree A))
```

Create values by applying constructors: `Some 42`, `Err "nope"`, `Valid "ok"`, `Node 1 [Node 2 [], Node 3 []]`.
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

### Opaque types

```aivi
opaque Url = {
  protocol: Text, host: Text, port: Option Int,
  path: Text, query: List (Text, Text), hash: Option Text
}
```

The `opaque` keyword hides the type's internal structure outside its defining module:
- **Inside the module**: fully transparent — construct, access fields, pattern match, update freely.
- **Outside the module**: opaque — no record literals, no field access, no `<|` update, no structural pattern matching.
- Domain operators and exported functions still work from outside.
- Works for records, ADTs (`opaque Color = Red | Green | Blue`), branded types, and plain aliases.

### Closed records

```aivi
User = { id: Int, name: Text, email: Option Text }

// Functions require the exact record shape:
greet : User -> Text
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
Defaulted (email) User   // like Optional at type level; used for codec/default derivation

// Type-level pipe:
User |> Pick (id, name) |> Optional (name)
```

### Type signatures

```aivi
add : Int -> Int -> Int
```

Type signatures are also required for multi-clause function definitions (`f = | ... => ...`).

### Classes and instances (ad-hoc polymorphism, HKTs)

```aivi
class Setoid A = {
  equals: A -> A -> Bool
}

class Ord A = Setoid {
  lte: A -> A -> Bool
}

class Semigroup A = {
  concat: A -> A -> A
}

class Monoid A = Semigroup {
  empty: A
}

class Functor (F A) = {
  map: (A -> B) -> F B
}

class Apply (F A) = Functor {
  ap: F (A -> B) -> F B
}

class Applicative (F A) = Apply {
  of: A -> F A
}

class Chain (F A) = Apply {
  chain: (A -> F B) -> F B
}

class Monad (M A) = Applicative, Chain {}

class Foldable (F A) = {
  reduce: (B -> A -> B) -> B -> B
}

class Traversable (T A) = Functor, Foldable {
  traverse: (A -> F B) -> F (T B)
}

class Filterable (F A) = Functor {
  filter: (A -> Bool) -> F A
}

class Alternative (F A) = Applicative {
  alt: F A -> F A
}

class Plus (F A) = Alternative {
  zero: F A
}

class Bifunctor (F A B) = {
  bimap: (A -> C) -> (B -> D) -> F C D
}

instance Monad (Option A) = { ... }
instance Monad (Result E A) = { ... }
instance Monad (List A) = { ... }
instance Semigroup Text = { ... }
instance Monoid Text = { ... }
```

Type variables in class/instance declarations are implicitly universally quantified.
Use `given (A: ClassName)` only when a real constraint is needed (e.g., `given (A: Eq)`).
HKT class member signatures use **abbreviated form**: the container type is omitted and
added internally by the compiler as the last argument.

All class methods from `aivi.logic` support both direct application and pipe syntax:
`map f xs` and `xs |> map f` are equivalent. `use aivi.logic` brings all class methods
into scope.

**Bidirectional resolution of zero-argument members**: Class members that are values
(e.g. `empty` from `Monoid`) resolve via expected type from context — a type annotation
or function signature is enough. When ambiguous, use qualified forms: `List.empty`,
`Map.empty`.

### Type class instance table

| Type | Setoid | Ord | Semigroup | Monoid | Functor | Filterable | Foldable | Traversable | Apply | Applicative | Chain | Monad | Bifunctor | Alternative | Plus |
|------|:------:|:---:|:---------:|:------:|:-------:|:----------:|:--------:|:-----------:|:-----:|:-----------:|:-----:|:-----:|:---------:|:-----------:|:----:|
| `List A` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ |
| `Option A` | ✓ | ✓ | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — | ✓ | ✓ |
| `Result E A` | ✓ | — | — | — | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | — |
| `Map K V` | ✓ | — | ✓ | ✓ | ✓ | ✓ | ✓ | — | — | — | — | — | — | — | — |
| `Generator A` | — | — | — | — | ✓ | ✓ | ✓ | — | — | — | — | — | — | — | — |
| `Stream A` | — | — | — | — | ✓ | ✓ | — | — | — | — | — | — | — | — | — |

### Type variable constraints

```aivi
class Collection (C A) = given (A: Eq) {
  elem : A -> C A -> Bool
  unique : C A -> C A
}
```

### Expected-type coercions

In positions where a `Text` is expected, the compiler may insert `toText expr` if a `ToText A` instance is in scope.

When the expected type is `Body` (from `aivi.net.http`), the compiler coerces:

- Record literal → `Json (toJson record)`
- `Text` → `Plain text`
- `JsonValue` → `Json jv`

When the expected type is `Option A` and the expression does not match, the compiler tries to coerce to `A` and wraps in `Some`. These coercions chain, so a bare record in an `Option Body` position becomes `Some (Json (toJson { ... }))`:

```aivi
body: { grant_type: "authorization_code", code: code }
// elaborated to: Some (Json (toJson { grant_type: ..., code: ... }))
```

`toJson : A -> JsonValue` is a structural built-in that converts any value — records, lists, primitives, `Option` — to a `JsonValue`. The `Json` body variant also auto-injects `Content-Type: application/json`.

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
- Predicates do not automatically unwrap `Option` or `Result`; use a pattern predicate or a full `match`.

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

AIVI has five block forms, each introduced by a keyword and delimited with `{ ... }`: `generate`, `do M`, `resource`, `machine`, and plain `{ ... }` (pure computation). The first four are described below; `resource` is covered in §11.

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
- `unless cond <- eff` - negated conditional (runs `eff` only when `cond` is false)
- `given cond or failExpr` - precondition guard
- Final expression must be `Effect E A` (commonly `pure value`)
- Statement expressions must be `Effect E Unit`; non-Unit results must be bound

**Tail-recursive loops in effect blocks:**

`loop`/`recurse` works inside `do Effect { ... }` blocks for stateful iteration:

```aivi
dijkstra = source graph => do Effect {
  dists0 = empty |> insert source 0.0

  loop state = { dists: dists0, pq: Heap.push (0.0, source) Heap.empty } => {
    dists = state.dists
    pq = state.pq
    result = Heap.popMin pq
    result match
      | None                       => pure dists
      | Some ((d, node), restPq)   => do Effect {
          currentDist = dists |> getOrElse node 999999.0
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

`do Monad { ... }` is the general form; `do Effect { ... }` is the most common specialisation. `Option`, `Result`, and `Query` are also supported:

```aivi
// do Option: short-circuits on None
safeLookup = key1 key2 map => do Option {
  x <- map |> get key1
  y <- map |> get key2
  Some (x + y)
}

// do Result: short-circuits on Err
validateAge = input => do Result {
  n <- parseInt input
  ok <- if n >= 0 && n <= 150 then Ok n else Err "out of range"
  Ok ok
}

// do Query: compose typed database queries (aivi.database)
// The shipped portable subset lowers to SQL; other helper-built queries keep legacy runtime semantics.
activeNames : Query Text
activeNames = do Query {
  user <- db.from userTable    // bind each row
  db.guard_ user.active        // skip rows where active is False
  db.queryOf user.name         // project the name field
}

// Execute against a connection
names <- db.runQueryOn conn activeNames
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

`-> State : init {}` marks the starting state. Machine values begin in that target state; the init edge is not a normal runtime step you call later. `Source -> Target : event { payload }` defines transitions with optional typed payloads. States are inferred from the transition graph. The compiler checks completeness and type safety.

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

### Negated conditional effects (`unless`)

`unless cond <- eff` runs `eff` only when `cond` is **false**. It is the exact negation of `when`.

```aivi
main = do Effect {
  cfg <- loadConfig
  unless cfg.skipSetup <- runSetup cfg
  unless cfg.quiet     <- print "done"
  process cfg
}
```

Desugars to: `_ <- if cond then pure Unit else eff`.

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
export add = a b => a + b                // inline exported binding
export machine Flow = {                  // inline exported declaration
  -> Idle : boot {}
}
```

### Prelude

Every module implicitly does `use aivi.prelude`. Disable with `@no_prelude`.

### Module path convention

- `vendor.name.*` - third-party libraries
- `user.app.*` - application code
- In application code, module path segments and backing file names should be `snake_case`.

**Core & Utils** (`aivi.*`):
`prelude` (implicit), `text`, `logic`, `units`, `regex`, `testing`, `collections` (re-exports `list`, `map`, `set`, `queue`, `heap`), `i18n`, `generator`, `json`, `defaults`

**Math & Science** (`aivi.*`):
`math`, `vector`, `matrix`, `number`, `geometry`, `graph`, `linear_algebra`, `tree`

**Time** (`aivi.chronos.*`):
`instant`, `calendar`, `duration`, `timezone`, `scheduler`

**System** (`aivi.*`):
`system`, `concurrency`, `crypto`, `secrets`, `log`

**IO** (`aivi.*`):
`file`, `console`, `database`, `database.pool`, `email`, `path`, `url`, `rest`

`aivi.database` supports both a default configured backend (`configure`, `load`, `applyDelta`) and
explicit `DbConnection` handles (`connect`, `open`, `loadOn`, `applyDeltaOn`). Prefer
`beginTxOn` / `inTransactionOn` / savepoint `...On` helpers for transaction-safe pooled code.

**Typed mutation helpers:** `db.insertOn`, `db.deleteWhereOn`, `db.updateWhereOn`, `db.upsertOn`
(and ambient `db.insert`, `db.deleteWhere`, `db.updateWhere`, `db.upsert`) are convenience
wrappers that construct the appropriate `Delta A` and call `applyDeltaOn` / `applyDelta` in one
step.  In v0.1 they execute **in memory** — they do not compile to SQL DML statements.

**Query DSL (v0.1):** `aivi.database` also exports a `Query A` type and `do Query { ... }`
notation for composing typed, composable queries. The portable subset (`db.from`,
`db.where_`, `db.guard_`, `db.select`, `db.orderBy`, `db.limit`, `db.offset`,
`db.count`, `db.exists`, and `do Query` blocks built from those forms) now lowers to a
SQL-backed plan when every participating table has an explicit column list. Those same
static schemas also let the checker catch missing row fields and obvious bad
projection/join field references early. Use `runQueryOn conn q` to execute against an
explicit connection, or `runQuery q` to execute against the default connection
configured with `db.configure`.

```aivi
// Build a typed query
expensiveItems : Query Text
expensiveItems = do Query {
  item <- db.from itemTable
  db.guard_ (item.price > 100)
  db.queryOf item.name
}
// Or with functional helpers
expensiveItems2 : Query Text
expensiveItems2 = db.from itemTable |> db.where_ (_.price > 100) |> db.select _.name

// Execute against an explicit connection
names <- db.runQueryOn conn expensiveItems

// Execute against the default connection (configured with db.configure)
names <- db.runQuery expensiveItems
```

Helper-built queries that do not lower still use the older in-memory `Query` runtime.
Unsupported `do Query` shapes do not silently fall back; today they surface a query
error when run, so keep `do Query` blocks to plain `from` binds, `guard_` filters,
simple `=` let-bindings, and a final `queryOf`/helper around it.

**Sorting and paging (v0.1):** `orderBy`, `limit`, and `offset` compile to SQL
`ORDER BY` / `LIMIT` / `OFFSET` inside the lowered subset. Queries outside that subset
keep the older in-memory sort/slice behavior.

```aivi
// Take 5 active users sorted by creation time, skipping the first 10
page : Query Text
page =
  db.from userTable
  |> db.where_ _.active
  |> db.orderBy _.createdAt
  |> db.offset 10
  |> db.limit 5
  |> db.select _.name
```

**Multi-table join (v0.1 portable subset):** use repeated `from` binds with `guard_`
in a `do Query` block. Inside the lowered subset this becomes a SQL cross join plus
pushed-down `WHERE` predicates. Current limits: each bind must still be a plain table
source; explicit join syntax, outer joins, grouping, and correlated subqueries are not
shipped yet.

```aivi
activeUserOrders : Query { user: User, order: Order }
activeUserOrders = do Query {
  user  <- db.from userTable
  db.guard_ user.active
  order <- db.from orderTable
  db.guard_ (order.userId == user.id)
  db.queryOf { user: user, order: order }
}
```

**`db.count` / `db.exists` (v0.1):** both helpers are available now. In the lowered
subset `db.count` emits SQL `COUNT(*)`, and `db.exists` emits a SQL existence probe
(`SELECT 1 ... LIMIT 1`-style). Outside the lowered subset they keep the older
in-memory behavior; they do not make an arbitrary query lowerable.

**Network** (`aivi.net.*`):
`http`, `https`, `httpServer`, `sockets`, `streams`

**UI** (`aivi.ui.*` and `aivi.*`):
`ui.gtk4`, `ui.layout`, `color`

---

## 13 External Sources

`Source K A` represents typed external data. Prefer **schema-first source declarations** and keep
`load` as the only effectful step inside `do Effect { ... }`.

```aivi
User = { id: Int, name: Text }

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json"
    schema: source.schema.derive
  }

appConfig : Source Env { port: Int, debug: Bool }
appConfig =
  env.decode {
    prefix: "AIVI_APP"
    schema: source.schema.derive
  }

do Effect {
  users <- load usersSource
  cfg   <- load appConfig
  pure (users, cfg)
}
```

Available source APIs in v0.1: `file.read/json/csv/imageMeta/image`, `http`/`https`, `rest`, `env.get/decode`, `email.imap`.

### Source pipeline helpers

Phase 3 adds pure source-pipeline combinators around the declaration:

```aivi
nonEmpty : List A -> Validation (List DecodeError) (List A)
nonEmpty = xs =>
  if List.length xs == 0 then
    Invalid [{ path: [], message: "expected at least one row" }]
  else
    Valid xs

usersCount : Source File Int
usersCount =
  usersSource
    |> source.transform List.length

validatedUsers : Source File (List User)
validatedUsers =
  usersSource
    |> source.validate nonEmpty
```

- `source.transform` is for pure normalization after decode.
- `source.validate` is for semantic rejection that should surface as `DecodeError`.
- `source.decodeErrors : SourceError K -> List DecodeError` extracts structured schema/validation mismatches (`IOError` becomes `[]`).
- The wider Phase 3 composition model adds canonical retry/timeout/cache/provenance stages around `load`; see `specs/syntax/external_sources/composition.md` for the stage order and policy semantics.

Compatibility forms like `file.json "./users.json"` and `env.decode "AIVI_APP"` still work, but the record forms above are the preferred public surface because tooling can describe the schema contract before `load`.

### Tooling notes

- Schema-first record declarations power hover/diagnostics for `file.json`, `env.decode`, `source.transform`, `source.validate`, `source.decodeErrors`, and `source.schema.derive`.
- `aivi lsp` checks whole **workspace snapshots** incrementally: open documents shadow disk, cached checkpoints reuse only when their fingerprints match, and dependents recheck when export surfaces or exported schema summaries change.
- AIVI comes with own MCP server 'aivi mcp serve --ui --allow-effects' you can use that to debug GTK interfaces and read docs.

### Email Module (`aivi.email`)

**Auth**: `EmailAuth = Password Text | OAuth2 Text` — supports XOAUTH2 for Gmail/Outlook.

**One-shot**: `imap : ImapConfig -> Effect Text (List A)` — connect, fetch, disconnect.
**SMTP**: `smtpSend : SmtpConfig -> Effect Text Unit` — multi-recipient with CC/BCC.
**MIME**: `mimeParts : Text -> List MimePart`, `flattenBodies : List MimePart -> Text`.

**Session API** (persistent connection via `Resource`):
```
imapOpen      : ImapConfig -> Resource Text ImapSession
imapSelect    : Text -> ImapSession -> Effect Text MailboxInfo
imapExamine   : Text -> ImapSession -> Effect Text MailboxInfo
imapSearch    : Text -> ImapSession -> Effect Text (List Int)
imapFetch     : List Int -> ImapSession -> Effect Text (List A)
imapSetFlags  : List Int -> List Text -> ImapSession -> Effect Text Unit
imapAddFlags  : List Int -> List Text -> ImapSession -> Effect Text Unit
imapRemoveFlags : List Int -> List Text -> ImapSession -> Effect Text Unit
imapExpunge   : ImapSession -> Effect Text Unit
imapCopy      : List Int -> Text -> ImapSession -> Effect Text Unit
imapMove      : List Int -> Text -> ImapSession -> Effect Text Unit
imapListMailboxes   : ImapSession -> Effect Text (List MailboxInfo)
imapCreateMailbox   : Text -> ImapSession -> Effect Text Unit
imapDeleteMailbox   : Text -> ImapSession -> Effect Text Unit
imapRenameMailbox   : Text -> Text -> ImapSession -> Effect Text Unit
imapAppend    : Text -> Text -> ImapSession -> Effect Text Unit
imapIdle      : Int -> ImapSession -> Effect Text IdleResult
```

Types: `MailboxInfo = { name: Text, separator: Option Text, attributes: List Text }`, `IdleResult = TimedOut | MailboxChanged`.

`@static` embeds sources at compile time: `@static schema = file.json "schema.json"` or `@static envName = env.get "AIVI_BUILD_ENV"`.

`@static` can also generate typed, callable API clients from OpenAPI specs:

```aivi
@static
petStoreApi = openapi.fromUrl ~url(https://petstore.example.com/v2/swagger.json)

@static
internalApi = openapi.fromFile "./specs/api.yaml"
```

The result is a factory function: pass a config record to get callable endpoint functions.

```aivi
client = petStoreApi { bearerToken: Some "sk-...", baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }
pets <- client.listPets { limit: Some 10 }

// Destructuring works too
{ listPets } = petStoreApi { bearerToken: None, baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }
result <- listPets {}
```

Config fields: `bearerToken : Option Text`, `headers : Option (List (Text, Text))`, `timeoutMs : Option Int`, `retryCount : Option Int`, `strictStatus : Option Bool`, `baseUrl : Option Text`.

`@static type.jsonSchema TypeName` generates an OpenAI-compatible JSON Schema from a type alias at compile time:

```aivi
ExtractionResult = {
  title:   Text,
  summary: Text,
  tags:    List Text,
  score:   Option Float
}

@static
extractionSchema = type.jsonSchema ExtractionResult
schemaText = toText extractionSchema
```

`extractionSchema` becomes an embedded schema value; use `toText extractionSchema` when another system expects the JSON document itself. Maps `Text`→string, `Int`→integer, `Float`→number, `Bool`→boolean, `List T`→array, records→object, `Option T`→nullable. Useful for LLM structured-output APIs.

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
~dt(2024-05-21T12:00:00Z)      // DateTime
~tz(Europe/Paris)              // TimeZone
~zdt(2024-05-21T12:00:00[Europe/Paris]) // ZonedDateTime
~k"app.button.save"            // i18n key (validated at parse time)
~m"Hello, {name}!"             // i18n message template (validated at parse time)
~`raw text, no interpolation`  // Raw Text — multiline, no { } interpolation
~<html><div>{ "x" }</div></html> // Typed VDOM node
~<gtk><object class="GtkBox" /></gtk> // Typed GTK builder node
~<gtk><GtkBox spacing="24" /></gtk> // Shorthand widget syntax
```

`~map{...}`, `~set[...]`, `~mat[...]`, `~<html>...</html>`, and `~<gtk>...</gtk>` are **structured** (parsed as AIVI expressions). Other sigils are raw text until the closing delimiter.

The backtick sigil `` ~`...` `` produces a `Text` value with the verbatim content between the backticks. It supports multiple lines and no `{ }` interpolation occurs:

```aivi
json   = ~`{"id": 1, "name": "Alice"}`
query  = ~`SELECT *
           FROM users
           WHERE id = 1`
indent = ~`
           | Hallo
           | Andreas
`
styles = ~`css
  | .myClass {
  |   color: red;
  | }
`
```

If every non-empty line in a multiline raw-text sigil starts with optional indentation followed by `|`, AIVI strips that indentation, removes the `|`, and drops one optional space after it. In that margin mode, an opening blank line and the final blank line before the closing backtick are removed as well.

The VSCode extension recognizes the first line of a multiline raw-text sigil as an embedded-language header when it is one of `css`, `html`, `xml`, `json`, `sql`, `js`, `javascript`, `ts`, or `typescript`. That header is editor metadata only and is not part of the resulting `Text`.

GTK sigils support **widget shorthand**: tags starting with `Gtk`, `Adw`, or `Gsk` are sugar for `<object class="...">` where attributes become props, bindings, or callback hooks automatically:

```aivi
// Shorthand (preferred)
state = signal { count: 0 }
title = state |> map (_.count) |> map "Count {_}"
saveCounter = event (do Effect {
  current = get state
  persistCount current.count
})

view = ~<gtk>
  <GtkBox spacing="24" marginTop="12">
    <GtkLabel label={title} />
    <GtkButton label="Increment" onClick={_ => update state (patch { count: _ + 1 })} />
    <GtkButton label="Save" onClick={saveCounter} />
  </GtkBox>
</gtk>

// Equivalent verbose form
view = ~<gtk>
  <object class="GtkBox" props={{ spacing: 24, marginTop: 12 }}>
    <object class="GtkLabel" props={{ label: title }} />
    <object class="GtkButton" props={{ label: "Increment" }} onClick={_ => update state (patch { count: _ + 1 })} />
    <object class="GtkButton" props={{ label: "Save" }} onClick={saveCounter} />
  </object>
</gtk>
```

Signals are first-class reactive values. Create source signals with `signal`, derive more signals with `derive` or `combineAll`, and mutate them with `set` or `update`:

```aivi
state = signal { count: 0, query: "" }
title = derive state (s => "Count {s.count}")
canSearch = combineAll { st: state, running: searchEvent.running } (vals =>
  vals.st.query != "" and not vals.running
)

update state (patch { count: _ + 1 })
update state (patch { query: "gtk" })
```

Event attrs accept either runtime functions or event-handle values. `onClick={handler}` installs the function directly; `onClick={saveEvent}` triggers the event handle directly. Event handles are created with `do Event { ... }` and expose reactive fields such as `result`, `error`, `done`, and `running`.

GTK sigils also support signal sugar in v0.1:

```aivi
~<gtk>
  <GtkButton onClick={_ => update state (patch { count: _ + 1 })} />
  <GtkEntry onInput={txt => set query txt} />
  <GtkEntry onActivate={submitSearch} />
  <GtkCheckButton onToggle={active => set enabled active} />
  <GtkScale onValueChanged={value => set volume value} />
  <GtkEntry onFocusIn={_ => set focused True} onFocusOut={_ => set focused False} />
  <GtkButton>
    <signal name="clicked" on={saveEvent} />
  </GtkButton>
</gtk>
```

`GtkSignalEvent` remains the low-level queue/event type used by `signalStream`, `signalPoll`, and tests:

```aivi
GtkSignalEvent =
  | GtkClicked       WidgetId Text
  | GtkInputChanged  WidgetId Text Text
  | GtkActivated     WidgetId Text
  | GtkToggled       WidgetId Text Bool
  | GtkValueChanged  WidgetId Text Float
  | GtkKeyPressed    WidgetId Text Text Text
  | GtkFocusIn       WidgetId Text
  | GtkFocusOut      WidgetId Text
  | GtkUnknownSignal WidgetId Text Text Text Text
```

For custom integrations and library code, consume raw events via `signalStream` (preferred) or `signalPoll`:

```aivi
events <- signalStream {}
concurrency.forEach events (event =>
  event match
    | GtkClicked _ _          => handleSave
    | GtkInputChanged _ _ txt => handleInput txt
    | GtkToggled _ _ active   => handleToggle active
    | _                       => yield {}
)
```

`concurrency.fold` threads state over a channel: `fold : s -> (s -> a -> Effect e s) -> Recv a -> Effect e s`.
`concurrency.forEach` runs an action on each event: `forEach : Recv a -> (a -> Effect e Unit) -> Effect e Unit`.
Both are exported from `aivi.concurrency`.

`buildWithIds` builds a widget tree and returns `{ root: WidgetId, widgets: Map Text WidgetId }` — useful for tests or low-level integrations that need direct widget ids.

A standard GTK app now exports a root `GtkWindow`/`GtkApplicationWindow` tree directly. The host mounts that tree once and keeps bound props, text nodes, classes, and structural child scopes live from signal writes. `<show>` and `<each key={...}>` are mounted structural bindings rather than full-tree rerenders.

For forms, keep `aivi.ui.forms.Field` values inside a signal or a record-valued signal, update them directly from `onInput`/`onFocusOut`, derive visible errors as signals, and let submit IO live in an `Event` handle.

Dynamic child lists are supported with keyed `<each>` bindings:

```aivi
~<gtk>
  <GtkBox>
    <each items={items} as={item} key={item => item.id}>
      <GtkLabel label={item.name} />
    </each>
  </GtkBox>
</gtk>
```

Component-style tags (uppercase/dotted) use **record-based lowering** in both sigils — attributes become record fields, children become a `children` field. Signal sugar and `props` normalization do not apply to component tags:

```aivi
// HTML component: Ui.Card { title: "Hello", children: [vElement "span" ...] }
~<html><Ui.Card title="Hello"><span>Body</span></Ui.Card></html>

// GTK component: Ui.Row { id: "r1", onClick: saveEvent }
~<gtk><Ui.Row id="r1" onClick={saveEvent} /></gtk>
```

GTK sigils also support **function-call tags** for local lowerCamel helpers. A simple uppercase self-closing tag with positional arguments lowers by lowercasing the first letter of the tag name:

```aivi
// Equivalent to: { navRailNode activeSection "sidebar" }
~<gtk><NavRailNode activeSection "sidebar" /></gtk>
```

Function-call tags do not use component record lowering, cannot mix positional arguments with attributes, and must be self-closing.

---

## 15 Decorators (v0.1)

Compile-time metadata only. No user-defined decorators.

| Decorator                                      | Purpose                                   |
|:---------------------------------------------- |:----------------------------------------- |
| `@test "desc"`                                 | Mark as test case (mandatory description) |
| `@static`                                      | Embed at compile time                     |
| `@native "mod.fn"`                             | Bind typed def to runtime/native path     |
| `@native "crate::path::fn"`                    | Bridge to Rust crate function (AOT only)  |
| `@deprecated`                                  | Emit warning on use                       |
| `@debug` / `@debug(pipes, args, return, time)` | Debug tracing (with `--debug-trace`)      |
| `@no_prelude`                                  | Skip implicit `use aivi.prelude`          |

`@static` supported sources: `file.read/json/csv`, `env.get`, `openapi.fromUrl ~url(...)`, `openapi.fromFile "..."`, `type.jsonSchema TypeName`. OpenAPI sources produce a factory function `Config -> { endpoints... }` where each endpoint is callable. `type.jsonSchema` produces an embedded OpenAI-compatible JSON Schema value; render it with `toText` when a downstream API expects JSON text.

Unknown decorators are compile errors.
`@native` is only valid on top-level definitions and requires an explicit type signature. No dummy body is needed — the compiler auto-generates the def from the type signature. Runtime natives use `.` paths (`"mod.fn"`); crate natives use `::` paths (`"crate::fn"`) and require `aivi build`.
Current limitation: `@debug` may still reject ordinary `name = x => ...` definitions with `E2010`; prefer the forms exercised by the decorator docs until resolver/HIR alignment lands.

---

## 16 Mock Expressions (Testing)

`mock ... in` scopes a binding substitution for testing without restructuring production code.

```aivi
mock <qualified.path> = <expr>
in <body>
```

**Deep scoping**: any function called inside `body` that internally uses the mocked binding sees the mock, not the original. This is what makes it useful — no dependency injection needed.

```aivi
use aivi.rest
use aivi.testing

fetchUsers = rest.get ~u(https://api.example.com/users)

@test "mock rest.get"
testFetch =
  mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
  in do Effect {
    users <- fetchUsers   // calls mock transparently
    assertEq (length users) 1
  }
```

**Multiple mocks** stack before `in`:

```aivi
mock rest.get  = _ => pure [{ id: 1, name: "Ada" }]
mock rest.post = _ _ => pure { success: True }
in do Effect { ... }
```

**Nested mocks** re-shadow outer mocks:

```aivi
mock rest.get = _ => pure []
in mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
   in do Effect { ... }   // sees inner mock
```

| Rule                 | Detail                                                                  |
|:-------------------- |:----------------------------------------------------------------------- |
| Only qualified paths | `mock rest.get = ...` ✓ — `mock localFn = ...` ✗ (use a local binding/helper for locals) |
| Type-safe            | Mock expression must match the original binding's type                  |
| Scoped               | Mock is only active inside `in <body>` — originals restored after       |
| Composable           | Works in any expression position, not just `@test`                      |

### Snapshot mocks

`mock snapshot <path>` records real responses on first run and replays from `.snap` files:

```aivi
@test "fetch users (snapshot)"
snapshotTest =
  mock snapshot rest.get
  in do Effect {
    users <- fetchUsers
    assertEq (length users) 3
  }
```

| CLI command                    | Behaviour                             |
|:------------------------------ |:------------------------------------- |
| `aivi test`                    | Replay from `.snap` — fail if missing |
| `aivi test --update-snapshots` | Re-record from real calls             |

### Snapshot assertions

`assertSnapshot` compares a value against a stored `.snap` file:

```aivi
assertSnapshot : Text -> A -> Effect Text Unit

@test "user table"
tableTest = do Effect {
  formatted <- pure (formatUserTable users)
  assertSnapshot "user_table" formatted
}
```

---

## 17 Domains, Units, and Operators

Domains define operator semantics and suffix literals for non-`Int` types.

```aivi
use aivi.chronos.duration (domain Duration)

deadline = { millis: 0 } + 10min     // + resolved by Duration domain
```

### Suffix literals

Suffix literals are numeric literals followed immediately by a suffix identifier. They resolve to domain-defined **template functions** named `1{suffix}`:

```aivi
10min  30s  100px  50%  1d  1y
```

Suffix can also be applied to a parenthesized expression (variable suffix):

```aivi
(x)kg       // desugars to 1kg applied to x; parentheses required, no space before suffix
```

Common suffix → domain mapping:

| Suffix                     | Domain   | Type            |
|:-------------------------- |:-------- |:--------------- |
| `10ms`, `1s`, `5min`, `2h` | Duration | `Duration`      |
| `1d`, `1m`, `1y`           | Calendar | `CalendarDelta` |
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
export domain Color over Rgb = { ... }         // inline exported domain declaration
```

### Built-in vs domain-resolved operators

Domain-resolved (when non-`Int`): `+`, `-`, `*`, `×`, `/`, `%`.
Built-in for `Int`, `Float`, `BigInt`, `Decimal`, and `Text`: `<`, `<=`, `>`, `>=` (lexicographic / Unicode codepoint order for `Text`).
Always built-in: `==`, `!=`, `&&`, `||`, `|>`, `<|`, `..`.

**Domains are not implicit casts.** They supply operator semantics and literal templates only. No global coercions are introduced by importing a domain.

---

## 18 Operator Precedence (lowest to highest)

1. `|>` (pipe)
2. `??` (coalesce Option)
3. `||` (logical or)
4. `&&` (logical and)
5. `==`, `!=` (equality)
6. `<`, `<=`, `>`, `>=` (comparison)
7. `+`, `-`, `++` (add, concat)
8. `*`, `×`, `/`, `%` (multiply)
9. `<|` (patch)

Unary prefix: `!` (not), `-` (negate).

> **Note:** `~` is **not** a unary operator; it is exclusively a sigil prefix (e.g. `~u(...)`, `~r/.../`).
> Bitwise operators (`&`, `|`, `^`, `~`, `<<`, `>>`) are not part of AIVI syntax. Use `aivi.bits` instead.

---

## 19 Complete Example

```aivi
@no_prelude
module integrationTests.complex.TopologicalSort

use aivi
use aivi.testing
use aivi.list
use aivi.map

Graph = { nodes: List Int, adj: Map Int (List Int) }

neighbors = node graph => graph.adj |> get node match
  | Some ns => ns
  | None    => []

reverseList = xs => reverseGo xs []

reverseGo = list acc => list match
  | []        => acc
  | [h, ...t] => reverseGo t [h, ...acc]

buildIndegree : Graph -> Map Int Int
buildIndegree = graph => {
  start = initIndegree graph.nodes empty
  processNodes graph graph.nodes start
}

initIndegree = nodes acc => nodes match
  | []        => acc
  | [n, ...t] => initIndegree t (acc |> insert n 0)

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

## 20 Quick Idiom Reference

| Task                         | AIVI idiom                                                              |
|:---------------------------- |:----------------------------------------------------------------------- |
| Transform a list             | `xs \|> map f`                                                          |
| Filter a list                | `xs \|> filter (age > 18)`                                              |
| Find first match             | `xs \|> find (name == "Alice")`                                         |
| Handle Option                | `opt match \| Some x => x \| None => default`                           |
| Handle Result                | `res match \| Ok x => x \| Err e => handle e`                           |
| Provide default for Option   | `opt ?? default` or `opt \|> getOrElse default`                         |
| Provide default for Result   | `res \|> getOrElse default`                                             |
| Check Option state           | `isSome opt`, `isNone opt`                                              |
| Check Result state           | `isOk res`, `isErr res`                                                 |
| Transform Option             | `opt \|> map f \|> filter pred \|> chain g`                             |
| Transform Result             | `res \|> map f \|> mapErr g \|> chain h`                                |
| Accumulate errors            | `ap (ap (Valid f) v1) v2` (Validation applicative)                      |
| GTK form field state         | `use aivi.ui.forms; field ""`, `setValue txt field`, `touch field`      |
| Check Validation state       | `isValid v`, `isInvalid v`                                              |
| Validation to Result         | `v \|> toResult`                                                        |
| Result to Validation         | `r \|> fromResult`                                                      |
| Option to Result             | `opt \|> toResult "error msg"`                                          |
| Result to Option             | `res \|> toOption`                                                      |
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
| Mock a dependency in test    | `mock rest.get = _ => pure [...] in do Effect { ... }`                  |
| Bitwise AND                  | `use aivi.bits; and a b`                                                |
| Shift bits right             | `use aivi.bits; b \|> shiftRight 8`                                     |
| Test a single bit            | `use aivi.bits; get 0 b`                                                |

---

## 21 REPL (`aivi repl`)

`aivi repl` opens an interactive session with the prelude pre-loaded. It is the fastest way to try snippets without creating a project.

```bash
aivi repl            # full-screen TUI (default when stdin is a terminal)
aivi repl --plain    # plain read-eval-print, pipe-friendly
aivi repl --color    # force ANSI colour
aivi repl --no-color # disable ANSI colour
```

**Inside the session**

- Type any AIVI expression or definition and press Enter to evaluate.
- Successful expressions show their runtime value together with the inferred type, for example `4 :: Int`.
- Use Shift+Enter for multi-line input; Ctrl+D (on empty input) to exit.
- Ctrl+L clears the transcript. When you start with `/`, the TUI suggests matching slash commands; use `↑` / `↓` to move, `Tab` to accept, or let Tab toggle the symbol pane when no suggestion is shown.

**Key slash commands**

| Command | What it does |
| --- | --- |
| `/help` | Print all commands |
| `/use aivi.text` | Add a module import for this session |
| `/types`, `/values`, `/functions`, `/modules` | Browse symbols in scope |
| `/history [n]` | Show the last n inputs |
| `/load path/to/file.aivi` | Load a file into the session |
| `/clear` | Clear transcript (keep definitions) |
| `/reset` | Clear transcript and all session state |
| `/openapi file spec.yaml [as petstore]` | Inject OpenAPI spec file as a typed module |
| `/openapi url https://... [as petstore]` | Inject OpenAPI spec from URL as a typed module |

The `/openapi` commands inject `@static`-style bindings (typed per the OpenAPI schema) so you can call the API's operations immediately. The module name defaults to a slug of the spec's `info.title`; pass `as <name>` to override it.

---

## Anti-Patterns (Do NOT write these)

| Wrong                  | Why                              | Correct                                             |
|:---------------------- |:-------------------------------- |:--------------------------------------------------- |
| `let x = 1`            | No `let` keyword                 | `x = 1`                                             |
| `def f(x):`            | No `def`, no parens for args     | `f = x => ...`                                      |
| `fn f(x: T) -> R`      | No `fn` keyword                  | `f : T -> R` / `f = x => ...`                       |
| `f :: T -> R`          | Single colon for type signatures | `f : T -> R`                                        |
| `var x = 1; x = 2`     | No mutation                      | `x = 1` (shadow with `x = x + 1` on next line)      |
| `x = 1; y = 2`         | No semicolons — use newlines     | `x = 1` (newline) `y = 2`                           |
| `do Effect` `{` on next line | Opening `{` must be on same line | `do Effect {`                                 |
| `null` / `nil`         | No nulls                         | `None` / `Option A`                                 |
| `Just x` / `Nothing`   | AIVI is not Haskell              | `Some x` / `None`                                   |
| `Left e` / `Right x`   | AIVI is not Haskell              | `Err e` / `Ok x`                                    |
| `Some(x)` / `Ok(x)`    | Constructors take no parens      | `Some x` / `Ok x`                                   |
| `throw` / `try/catch`  | No exceptions                    | `fail e` / `attempt` / `or`                         |
| `for x in xs { ... }`  | No loops                         | `xs \|> map f` or `generate { x <- xs; yield f x }` |
| `while cond { ... }`   | No loops                         | Recursion or `loop`/`recurse` in generators         |
| `f(x, y)`              | No parens for function calls     | `f x y`                                             |
| `x.method()`           | No methods, no parens            | `method x` or `x \|> method`                        |
| `List<Int>` / `Option<T>` | No angle-bracket generics     | `List Int` / `Option T`                             |
| `List.map f xs`        | HKT methods are unqualified      | `map f xs` (with `use aivi.logic`)                  |
| `opt \|> flatMap f`    | Option uses `chain`, not `flatMap` | `opt \|> chain f`                                 |
| `res \|> flatMap f`    | Result uses `chain`, not `flatMap` | `res \|> chain f`                                 |
| `fmap` / `>>=` / `<$>` / `<*>` | Haskell operators        | `map` / `chain` / `map` / `ap`                      |
| `Tree A = Leaf A \| Node (Tree A) (Tree A)` | `aivi.tree` is a rose tree | `Tree A = Node A (List (Tree A))`        |
| `impl Trait for Type`  | Rust syntax                      | `instance Class (Type) = { ... }`                   |
| `newtype Foo = Foo T`  | Haskell syntax                   | `opaque Foo = T`                                    |
| `do { x <- m }`        | Must name the monad              | `do Effect { x <- m }`                              |
| `case x of ...`        | `case` is kernel only            | `x match \| pat => expr`                            |
| `String`               | Type is called `Text`            | `Text`                                              |
| `return x`             | No return statement              | Expression result is implicit; `pure x` in effects  |
| `{ x = 1 }` in records | `=` is binding, not record field | `{ x: 1 }`                                          |
| `a & b`, `a \| b`      | No bitwise operators             | `use aivi.bits; and a b`, `or a b`                  |
| `a << 2`, `a >> 2`     | No shift operators               | `use aivi.bits; shiftLeft 2 a`, `shiftRight 2 a`    |
| `~a` (bitwise not)     | `~` is for sigils only           | `use aivi.bits; complement a`                       |
| `"x" ++ "y"`           | No string concat operator        | `"{x}{y}"`                                          |
| `import X`             | No `import` keyword              | `use module.path`                                   |
| `~t(12:00:00)`         | Verified time/date sigils use `~dt(...)` / `~zdt(...)` | `~dt(2024-05-21T12:00:00Z)` |
| `pure ()`              | Aivi uses Unit as ()             | `pure Unit`                                         |
| `use Aivi.List`        | Module paths are `snake_case`    | `use aivi.list`                                     |
| `x = 1 -- init`        | No `--` comments                 | use `//` or `/* .... */`                            |
| no \n after `=`        | Keep it on the same line         | `test = arg => arg match `                          | 
| no semicolons          | Aivi uses newline                | Separate statement with newline                     | 
| Deep nesting / inline lambdas in lambdas | Extract into named helpers | `step1 = ...; step2 = ...; result = step2 (step1 x)` |
