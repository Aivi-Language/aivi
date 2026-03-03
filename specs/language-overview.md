# Language at a Glance

A one-page reference for experienced users. For full details follow the section links.

---

## Core Design

- **Statically typed**, purely functional, expression-oriented
- Immutable bindings — no mutation, no loops, no `null`
- `Option A` / `Result E A` instead of null/exceptions
- Recursion, folds, or generators instead of loops
- `=` bindings must be **total**; refutable matches use `match`
- Records are **closed** and structural (no row polymorphism)
- Effects are explicit: `Effect E A` — `E` error type, `A` success type
- **Domains** give operator meaning and suffix literals for non-`Int` types

---

## Lexical Basics

| Element | Syntax |
|:--------|:-------|
| Line comment | `// to end of line` |
| Block comment | `/* ... */` — no nesting |
| Value / function name | `lowerCamelCase` |
| Type / constructor / domain name | `UpperCamelCase` |
| Module path segment / file name | `snake_case` |
| Text literal | `"hello { name }"` — interpolation via `{ expr }` |
| Numeric | `42`, `3.14`, `'a'` |
| ISO instant | `2024-05-21T12:00:00Z` |
| Suffixed number | `10px`, `30s`, `100%` — domain-resolved |
| Keywords | `as class do domain effect else export generate given hiding if in instance machine match mock module on or over patch recurse resource snapshot then unless use when with yield loop` |

`True`, `False`, `None`, `Some`, `Ok`, `Err` are constructors, not keywords.

---

## [Bindings](syntax/bindings)

```aivi
x = 42
add = a b => a + b
add : Int -> Int -> Int         // type signature

// Destructuring (must be total)
{ name, age } = user
(a, b) = pair
[h, ...t] = list
user as { name } = getUser      // whole value + destructure
{ data.user.profile.name } = record  // deep path
```

Shadowing is allowed — introduces a new binding, no mutation. Module-level bindings are mutually recursive.

---

## [Functions](syntax/functions)

```aivi
// Curried application
inc = add 1              // partial application
xs |> map inc |> filter (_ > 0)   // pipe: x |> f = f x

// Lambdas
x => x + 1              // standard
_ + 1                   // placeholder (unary only)

// Multi-clause (requires type sig)
describe : Int -> Text
describe =
  | 0 => "zero"
  | _ => "other"

// Accessor sugar
users |> map .name       // .field is x => x.field
```

---

## [Types](syntax/types/primitive_types)

```aivi
// Primitives
Unit  Bool  Int  Float

// Stdlib
Text  Bytes  Decimal  BigInt
Duration  Instant  Date  Time  TimeZone  ZonedDateTime

// ADTs
Option A = None | Some A
Result E A = Err E | Ok A
Tree A = Leaf A | Node (Tree A) (Tree A)

// Branded nominal
Email = Text!
mkEmail = text => Email text

// Closed record
User = { id: Int, name: Text, email: Option Text }

// Tuple / list / spread / range
pair = (1, "hello")
xs = [1, 2, 3]
ys = [0, ...xs, 4]
zs = [1 .. 10]             // inclusive range

// Row transforms (type-level)
Pick (id, name) User
Omit (isAdmin) User
Optional (email) User
Required (email) User
User |> Pick (id, name) |> Optional (name)

// Classes and HKTs
class Functor (F A) = given (A: Any) {
  map : (A -> B) -> F B
}
instance Monad (Option A) = given (A: Any) { ... }
// Typeclass methods accessed via pipe only: fa |> map f
```

---

## [Pattern Matching](syntax/pattern_matching)

```aivi
value match
  | Ok x        => x
  | Err _       => 0

// Guards
n match
  | _ when n > 0  => "positive"
  | _ when n < 0  => "negative"
  | _              => "zero"

// Nested / constructor patterns
| Some (Ok { name }) => name

// Pipeline + match
input |> parse |> validate match
  | Ok x  => x
  | Err e => handle e
```

All `match` expressions must be exhaustive — compile error otherwise.

---

## [Predicates](syntax/predicates)

Auto-lifted `Bool` expressions where `A -> Bool` is expected:

```aivi
users |> filter active             // active = _.active
users |> filter (age > 18)         // _.age > 18
xs |> takeWhile (_ < 10)
!active && (age > 18 || role == "admin")
```

---

## [Patching](syntax/patching)

`<|` applies a type-checked structural update:

```aivi
user2 = user <| { name: "Sam" }
user3 = user <| { profile.avatar: "new.png" }
record <| { items[*].price: _ * 1.1 }          // all items
record <| { items[price > 80].tag: "hot" }      // predicate selector
record <| { lookup["key"]: newVal }             // map key selector
p = patch { name: toUpper }                     // patch-as-value
```

---

## [Effects](syntax/effects)

```aivi
main = do Effect {
  cfg  <- load (file.read "config.json")  // run effect
  name = cfg.appName                       // pure binding
  print "loaded { name }"                  // Effect E Unit
  pure {}                                  // final value
}

// Error fallback
txt <- file.read path or "(missing)"
val <- riskyOp or
  | NotFound _ => pure default
  | Timeout _  => fail "timed out"

// Conditional
when cfg.verbose <- print "verbose enabled"
unless cfg.quiet  <- print "done"

// Precondition guard
given amount > 0 or fail (InvalidAmount amount)

// Attempt (catch as Result)
res <- attempt (api.fetchUser id)
res match
  | Ok user => pure user
  | Err _   => pure GuestUser
```

| Operation | Type |
|:----------|:-----|
| `pure` | `A -> Effect E A` |
| `fail` | `E -> Effect E A` |
| `bind` (`<-`) | sequence |
| `attempt` | `Effect E A -> Effect F (Result E A)` |

---

## [Generators](syntax/generators)

Pull-based, lazy, pure:

```aivi
evens = generate {
  x <- [1 .. 100]
  x -> x % 2 == 0     // guard
  yield x
}

fibs = generate {
  loop (a, b) = (0, 1) => {
    yield a
    recurse (b, a + b)
  }
}
```

---

## [Resources](syntax/resources)

```aivi
managedFile = path => resource {
  handle <- file.open path
  yield handle           // provide to caller
  file.close handle      // cleanup (always runs)
}

main = do Effect {
  f <- managedFile "data.txt"   // acquired
  content <- file.readAll f
  print content
}                                // released (LIFO)
```

---

## [Domains](syntax/domains)

```aivi
domain Color = {
  (+) : Color -> Color -> Color
  (*) : Float -> Color -> Color
}

// Usage — operator semantics from domain
blend = c1 + c2
scaled = 0.5 * red
```

Domains resolve `+`, `-`, `*`, `×`, `/`, `%`, `<`, `<=`, `>`, `>=` for non-`Int` types.
Suffix literals (`10px`, `30s`) elaborate as template function application.

---

## [Machines](syntax/machines_runtime)

```aivi
machine Door = {
           -> Closed : init   {}
  Closed   -> Opened : open   {}
  Opened   -> Closed : close  {}
  Opened   -> Locked : lock   {}
  Locked   -> Closed : unlock {}
}

do Effect {
  { lease, run, done, can } = DoorMachine
  _ <- lease {}           // valid — Closed -> Opened
  _ <- can.close Unit     // Bool: is close valid now?
}
```

---

## [Modules](syntax/modules)

```aivi
module my.app.api
export fetchUser, User

use aivi.net.http (get)
use aivi.json (decode)
use aivi.chronos.duration (domain Duration)  // import domain
use aivi.text as T
use aivi.text hiding (trim)
```

One module per file. Module path segments and file names are `snake_case`.

---

## [External Sources](syntax/external_sources)

```aivi
cfg  <- load (file.read "config.json")
rows <- load (file.csv "users.csv")
resp <- load (rest.get ~u(https://api.example.com/data))
key  <- load (env.get "API_KEY")

@static schema = file.json "schema.json"          // compile-time embed
@static client = openapi.fromFile "./api.yaml"    // typed OpenAPI client
```

Sources: `file.read/json/csv/imageMeta/image`, `http`/`https`, `rest`, `env.get/decode`, `email.imap`.

---

## [Operators & Sigils](syntax/operators)

**Precedence** (low → high): `|>` · `??` · `||` · `&&` · `==`/`!=` · comparisons · `|` · `^` · shifts · `+`/`-`/`++` · `*`/`/`/`%` · `<|`

**Sigils:**

```aivi
~u(https://example.com)        // Url
~path[/usr/local/bin]          // Path
~r/[a-z]+/i                    // Regex
~map{ "a" => 1 }               // Map K V
~set[1, 2, 3]                  // Set A
~mat[1.0 0.0; 0.0 1.0]         // Matrix
~d(2024-05-21)                 // Date
~t(12:00:00)                   // Time
~tz(Europe/Paris)              // TimeZone
~<html><div>{ x }</div></html> // VNode
~<gtk><GtkButton label="Hi" onClick={ Msg.Click } /></gtk>  // GtkNode
```

---

## [Decorators](syntax/decorators)

| Decorator | Purpose |
|:----------|:--------|
| `@test "desc"` | Test case (mandatory description) |
| `@static` | Embed source at compile time |
| `@native "mod.fn"` | Bind to runtime/native function |
| `@deprecated "hint"` | Emit warning on use |
| `@debug` / `@debug(pipes, args, return, time)` | Tracing (requires `--debug-trace`) |
| `@no_prelude` | Skip implicit `use aivi.prelude` |

Unknown decorators are compile errors. `@native` requires a top-level explicit type signature.

---

## [Mock Expressions](syntax/decorators#mock-expressions)

Scoped dependency substitution for tests — no production code changes needed:

```aivi
@test "fetch returns users"
testFetch =
  mock rest.get = _ => pure [{ id: 1, name: "Ada" }]
  in do Effect {
    users <- fetchUsers
    assertEq (List.length users) 1
  }
```

---

## Standard Library — Quick Map

| Import | Key symbols |
|:-------|:------------|
| `aivi.prelude` | auto-imported; `Int`, `Text`, `List`, `Option`, `Result`, … |
| `aivi.option` | `isSome`, `isNone`, `fromOption`, `toList` |
| `aivi.result` | `isOk`, `isErr`, `fromResult`, `mapErr` |
| `aivi.text` | `toUpper`, `toLower`, `trim`, `split`, `contains`, `replace` |
| `aivi.logic` | `Functor`, `Monad`, `Applicative` instances |
| `aivi.collections` | `List`, `Map`, `Set`, `Queue`, `Heap` |
| `aivi.generator` | generator combinators |
| `aivi.regex` | `~r/.../flags`, `match`, `test`, `replace` |
| `aivi.validation` | `Validation`, applicative error accumulation |
| `aivi.json` | `decode`, `encode`, `toJson`, `JsonValue` |
| `aivi.math` | `abs`, `sqrt`, `floor`, `ceil`, `pow`, `log`, `sin`, `cos` |
| `aivi.chronos.duration` | `2h`, `30m`, `1s` — domain suffix literals |
| `aivi.chronos.instant` | `Instant`, `now`, ISO-8601 literal |
| `aivi.file` | `read`, `write`, `json`, `csv`, `open`, `readAll`, `close` |
| `aivi.console` | `print`, `println`, `readLine` |
| `aivi.concurrency` | `par`, `scope`, `spawn`, `race`, `timeoutWith`, channels |
| `aivi.net.http` | `get`, `post`, `fetch`, `Request`, `Response` |
| `aivi.net.httpServer` | `listen`, `route`, `Handler` |
| `aivi.ui.gtk4` | `gtkApp`, `signalStream`, `buildFromNode`, `reconcileNode` |
| `aivi.crypto` | `hash`, `hmac`, `encrypt`, `decrypt` |
| `aivi.database` | `connect`, `query`, `transaction` |
| `aivi.log` | `info`, `warn`, `error`, `debug` |
