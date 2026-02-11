# AIVI Language Specification and Reference

This document aggregates the full specification, standard library, and examples of the AIVI programming language.
It is intended as a comprehensive context for LLMs to understand the language and generate code.

# PART 1: SPECIFICATIONS



<!-- FILE: /01_introduction -->

# AIVI Language Specification (v0.1)

> Note: **AIVI is a fictional language that may or may not materialize.** This document is a design/spec exploration, not a promise of an eventual implementation.

## 0. Overview

AIVI is a statically typed, purely functional language designed for **high-integrity data pipelines** and **domain-driven design**.

### Core characteristics

**Logic**

* Global type inference
* Classes (ad-hoc polymorphism)
* Higher-Kinded Types (HKTs)

**Data**

* Immutable by default
* **Open structural records** (row polymorphism)
* Algebraic Data Types (ADTs)

**Control**

* Pattern matching
* **Predicate-driven transformations**
* **Pure generators**
* Fiber-based structured concurrency
* Explicit effect tracking with `Effect E A`
* **Declarative Resource Management**

**Intentional omissions**

* No loops (use recursion, folds, generators)
* No exceptions (use `Result`)
* No `null` / `undefined` (use `Option`)
* No string concatenation (use interpolation)

### Naming

* **Uppercase** identifiers → types and constructors
* **lowercase** identifiers → values and functions


## Normative Principles

> **Bindings are immutable.**
> **Patterns are total by default; use `?` for partial matches.**
> **Predicates are expressions with implicit scope (`.prop`).**
> **Patches describe structure, not mutation (`<|`).**
> **Domains own semantics and interpreted operators.**
> **Generators model data streams; effects model typed I/O (`Effect E A`).**

## Why AIVI?

AIVI is designed to solve the complexity of modern data-heavy applications by shifting the focus from **how** data is moved to **what** data means. 

### High Integrity by Design
By eliminating `null`, exceptions, and mutable state, AIVI ensures that if a program compiles, it is fundamentally sound. Its exhaustive pattern matching and totality requirements for bindings make "unhandled state" a impossibility at the type level.

### Universal Portability (WASM & WASI)
AIVI is built from the ground up to target **WebAssembly (WASM)**. 
- **Browser**: High-performance client-side logic and Aivi LiveView-like frontends.
- **Server/Edge**: Using **WASI** (WebAssembly System Interface), AIVI runs in highly isolated, secure sandboxes across cloud and edge infrastructure with near-native speed and instant startup.
- **Security**: The WASM capability-based security model naturally complements AIVI's explicit effect tracking.

### The Power of Domains
In AIVI, the language doesn't try to know everything. Instead, it provides **Domains**—a mechanism to extend the language's semantics.
- **Semantic Arithmetic**: Operators like `+` and `-` are not restricted to numbers; they are interpreted by domains to perform calendar shifts, color blending, or vector math.
- **Syntactic Sugar**: Surface-level syntax can desugar into a small kernel, keeping the core language minimal.
- **Extensibility**: Developers can define their own domains, creating a language that speaks the vocabulary of their specific business area (Finance, IoT, UI) without losing the safety of the AIVI core.


This document defines **AIVI v0.1** as a language where **data shape, transformation, and meaning are explicit, uniform, and statically enforced**.


<!-- FILE: /roadmap/ -->

# AIVI Roadmap

This roadmap tracks the incremental development of the AIVI language, compiler, and tooling. Each phase produces something shippable and dogfoodable.

## Guiding Principles

- **Kernel First**: Implement the Kernel first, then desugar surface language into it (`specs/03_kernel` → `specs/04_desugaring`).
- **Unified Engine**: Single parser, name resolver, and typechecker for both Compiler and LSP.
- **First-Class Domains**: Design Domains, Effects, and Resources early.

---

## Phase M0: Repo + CI Scaffolding (Complete)

- [x] Rust workspace skeleton (`crates/*`) and single `aivi` binary.
- [x] `aivi --help` with subcommands: `parse`, `check`, `fmt`, `lsp`, `build`, `run`.
- [x] "Hello world" golden test for parsing.
- [x] `cargo test` runs in CI.

## Phase M1: Parser + CST + Diagnostics (Complete)

- [x] Lexing + parsing for `specs/02_syntax` (excluding types initially).
- [x] CST preserving trivia (comments/whitespace) for IDE/Fmt.
- [x] Structured diagnostics with spans/codes.
- [x] Formatter prototype (pretty printer).
- [x] VS Code syntax highlighting.

## Phase M2: Modules + Name Resolution (Complete)

- [x] `module ... = { export ... }` and `use` imports.
- [x] Symbol tables and module graph.
- [x] `aivi check` resolving identifiers across workspace.
- [x] LSP: `textDocument/definition` (in-file).
- [x] LSP: `textDocument/definition` across modules.

## Phase M3: Kernel IR + Desugaring (Complete)

- [x] Kernel definitions matching `specs/03_kernel/01_core_terms.md`.
- [x] Surface-to-kernel desugaring pipeline.
- [x] `aivi desugar` debug output.
- [x] HIR with stable IDs for IDE.
- [x] **Acceptance**: Round-trip tests for surface features.

## Phase M4: Type System v1 (Complete)

- [x] ADTs, functions, closed records, pattern matching.
- [x] `Option`, `Result`, `List`, `Text` as library types.
- [x] Let-generalization.
- [x] Minimal traits (`Eq`, `Ord`, `Show`, `Num`).
- [x] `aivi check` with type error traces and typed holes (`_`).
- [x] Canonical type pretty-printer.
- [x] LSP: Hover types, signature help.

## Phase M5: Execution (Rust Transpilation & Native Runtime) (Complete)

- [x] `aivi build --target rustc` emits binary via Rust transpilation.
- [x] `aivi run` executes program (native/interpreter).
- [x] Basic runtime support (heap, strings, lists).
- [x] **Acceptance**: Deterministic golden tests, memory safety.

---

## Phase M6: Effects, Resources, Concurrency (In Progress)

Scope: Implement `Effect E A` semantics, structured concurrency, and resource management.

- [x] Built-in effects: `Clock`, `File`, `Random` (partial).
- [x] `specs/06_runtime/01_concurrency.md` implementation (`scope`, `par`, `race`).
- [x] Cancellation propagation rules.
- [x] `bracket`/`with` resource pattern.
- [x] Deterministic cancellation semantics.

## Phase M7: Domains + Patching (Complete)

Scope: Domain definitions, operator overloading, and patching semantics.
- [x] Domain definitions and operator interpretation (`specs/02_syntax/11_domain_definition.md`).
- [x] Patching semantics (`specs/02_syntax/05_patching.md`).
- [x] Domain-specific numeric deltas (calendar/duration/color).
- [x] Built-in sigils as domain literals (`~r`, `~u`, `~d`, `~dt`) wired through lexer/parser → HIR/Kernel and editor tooling.

## Phase M8: LSP "Daily Driver" (Complete)

Scope: Make editing AIVI comfortable for daily work.

- [x] Diagnostics (syntax/type errors).
- [x] Definition (in-file).
- [x] Formatting (via `aivi fmt`).
- [x] References (find usages).
- [x] Rename refactoring.
- [x] Hover documentation & resolved types.
- [x] Semantic tokens.
- [x] Code actions (quick fixes).

## Phase M9: MCP Integration (Complete)

Scope: Expose AIVI modules as Model Context Protocol (MCP) tools/resources.

- [x] `aivi mcp serve` exposing `@mcp_tool` and `@mcp_resource`.
- [x] JSON Schema generation from AIVI types.
- [x] Capability gates for unauthorized effects.

## Phase M10: Type System v2 (Long Term)

Scope: Advanced typing features.

- [x] Row polymorphism (open records).
- [x] Type classes (ad-hoc polymorphism).
- [x] Higher-Kinded Types (HKTs).

---

## Detailed Plans

### Language Implementation Plan
1. **Concrete Syntax → CST**: Tokens, modules, bindings, ADTs, records, patterns. (Done)
2. **AST & Lowering**: CST→AST→HIR→Kernel pipeline. (Done)
3. **Modules**: Resolution, cycles, shadowing. (Done)
4. **Kernel IR**: The executable truth. (Done)
5. **Typechecking**: Monomorphic → Polymorphic → Traits → Effects. (Mostly Done)
6. **Diagnostics**: Error codes, labels, suggestions. (Ongoing)
7. **Patterns**: Exhaustiveness checking. (Planned)
8. **Predicates & Patching**: Central AIVI identity features. (Planned)
9. **Domains**: Custom literals and operators. (In Progress)

### Standard Library Plan
- **Phase 1**: Compiler intrinsics + thin wrappers (`aivi`).
- **Phase 2**: Implement stdlib in AIVI language where possible.
- **Phase 3**: Optimization (persistent collections, HAMT).
- **Modules**: `Prelude`, `Core` (Int/Float/Bool/Text/List), `Collections`, `Bytes`, `Json`.
- **Domains**: `Duration`, `Calendar`, `Color`, `Vector`.
- **Effects**: `Console`, `Clock`, `Random`, `File`, `Net`.

### Rust Workspace Layout
- **Binaries**: `aivi_cli`, `aivi_lsp`, `aivi_mcp`.
- **Core Libs**: `span`, `lexer`, `cst`, `parser`, `ast`, `hir`, `resolve`, `desugar`, `kernel`, `types`, `effects`.
- **Codegen**: `runtime`, `rust_codegen`.
- **Tooling**: `fmt`, `tests`, `doc`.

<!-- FILE: /02_syntax/01_bindings -->

# Bindings and Scope

## 1.1 Definitions

All bindings use `=`:

* values
* functions
* types
* classes
* instances
* modules

```aivi
pi = 3.14159
add = x y => x + y
```


## 1.2 Shadowing

Bindings are lexical and may be shadowed.

```aivi
x = 1
x = x + 1
```

This introduces a new binding; no mutation exists. This is common in functional languages like OCaml and Rust (re-binding) but distinct from mutation.

## 1.2.1 Recursion (module level)

Within a `module ... = { ... }` body, top-level value bindings are **recursive**: a binding may refer to itself and to bindings that appear later in the same module body.

This supports ordinary recursive functions:

```aivi
module demo.recursion = {
  export sum

  sum =
    | []        => 0
    | [h, ...t] => h + sum t
}
```

Local recursion inside `do { ... }` / `effect { ... }` blocks is a future surface feature; in v0.1, prefer defining recursive helpers at module scope.


## 1.3 Pattern Bindings

Structural patterns may appear in bindings.

```aivi
{ name } = user      // Shorthand for { name: name }
{ name: n } = user   // Rename binding to 'n'
[h, ...t] = xs       // List destructuring
```

* `=` may only be used where the compiler can prove the pattern is **total** (i.e., it covers all possible shapes of the data).
* Potentially failing matches (refutable patterns) must use `?` (case analysis) or appear in a context where failure can be handled.

> [!NOTE]
> Using `=` with a non-total pattern (like `[h, ...t] = []`) results in a compile-time error. For partial matches, use the `?` operator which converts a refutable pattern into an `Option` or branch.


## 1.4 Whole-value binding with `@`

Patterns may bind the **entire value** alongside destructuring.

```aivi
user@{ name: n } = input
user@{ name } = input
```

Semantics:

* `user` is bound to the whole value
* `{ name: n }` destructures the same value
* no duplication or copying occurs

Allowed in:

* Top-level and local bindings
* `?` pattern arms (allowing capture of the matched sub-structure)
* Function clauses 

Example:

```aivi
// u is bound to the full record; id/name come from destructuring
formatUser = u@{ id, name } => "{id}: {name}"
```


## 1.5 Usage Examples

### Config Binding

```aivi
config = {
  host: "localhost"
  port: 8080
  debug: True
}

{ host, port } = config
serverUrl = "http://{host}:{port}" 
```

### Tuple Destructuring

```aivi
point = (10, 20)
(x, y) = point

distance = sqrt ((x * x) + (y * y))
```

### Deep path destructuring

Record destructuring supports **dot-paths** to access nested fields directly. This combines path addressing with the `@` whole-value binder.

```aivi
{ data.user.profile@{ name } } = response
```

Semantics:
* `data.user.profile` is the path to the record being destructured.
* `@{ name }` binds the fields of that specific nested record.
* Intermediate records are **not** bound unless explicitly requested.

This is exactly equivalent to the nested expansion:
```aivi
{ data: { user: { profile: p@{ name } } } } = response
```
But much more readable for deep hierarchies.

> [!NOTE]
> Deep path destructuring is a powerful tool for working with complex JSON-like data, providing both brevity and clarity.

### List Head/Tail

```aivi
numbers = [1, 2, 3, 4, 5]
[first, second, ...rest] = numbers

// first = 1, second = 2, rest = [3, 4, 5]
```

### Function Definitions

```aivi
// Named function
greet = name => "Hello, {name}!"

// Multi-argument
add = x y => x + y

// With type annotation
multiply : Int -> Int -> Int
multiply = a b => a * b
```


<!-- FILE: /02_syntax/02_functions -->

# Functions and Pipes

## 2.1 Application

* Functions are **curried by default**
* Application is by whitespace

```aivi
add 5 10
```

---

## 2.2 Lambdas

`_` denotes a **single-argument lambda**.

```aivi
inc = _ + 1
```

Multi-argument lambdas must be explicit:

```aivi
add = x y => x + y
```

---

## 2.3 Pipes

Pipelines use `|>`.

```aivi
xs |> map inc |> filter (_ > 0)
```

---

## 2.4 Usage Examples

### Basic Functions

```aivi
// Identity
id = x => x

// Constant: returns a function that ignores its input and always returns x
const = x _ => x

// Flip arguments
flip = f => x y => f y x
 
// Function composition is most common via the pipe operator:
processName = name => name |> trim |> lowercase |> capitalize
result = processName "  HELLO  "
```


### Higher-Order Functions

```aivi
// Apply function twice
twice = f => x => f (f x)

increment = _ + 1
addTwo = twice increment

// Result: addTwo 5 = 7
```

### Partial Application

```aivi
add = x y => x + y
add5 = add 5

// add5 10 = 15

// With pipes
numbers = [1, 2, 3]
result = numbers |> map (add 10)
// [11, 12, 13]
```

### Block Pipelines


Pipelines allow building complex data transformations without nested function calls.

```aivi
users = [
  { name: "Alice", age: 30, active: True }
  { name: "Bob", age: 25, active: False }
  { name: "Carol", age: 35, active: True }
]

// Data processing pipeline
activeNames = users
  |> filter active
  |> map .name
  |> sort
  |> join ", "
// "Alice, Carol"

// Mathematical series
sigma = [1..100]
  |> filter (_ % 2 == 0)
  |> map (n => pow n 2)
  |> sum
```

### Expressive Logic: Point-Free Style

Functions can be combined to form new functions without naming their arguments, leading to very concise code.

```aivi
// Boolean logic composition
isAdmin = user => user.role == Admin
isOwner = user => user.id == ownerId
canDelete = user => isAdmin user || isOwner user

// Validation chains
isEmail = contains "@"
isLongEnough = x => x |> len |> (_ > 8)
isValidPassword = x => isEmail x && isLongEnough x

// Usage
passwords |> filter isValidPassword
```

### Lambda Shorthand

```aivi
// Single arg with _
double = _ * 2
isEven = _ % 2 == 0 
getName = .name // Accessor shorthand for x => x.name

// Equivalent explicit forms
double = x => x * 2
isEven = x => x % 2 == 0
getName = user => user.name

// Predicates can automatically deconstruct fields:
// filter active is a shortcut for filter (_.active)

// Complexity can be handled inline via explicit record deconstruction:
// map { name, id } => if id > 10 then name else "Anonymous"
```


<!-- FILE: /02_syntax/03_types -->

# The Type System

## 3.1 Primitive Types

AIVI distinguishes:

- **Compiler primitives**: types the compiler/runtime must know about to execute code.
- **Standard library types**: types defined in AIVI source (possibly with compiler-known representation in early implementations).

In v0.1, the recommended minimal set of **compiler primitives** is:

```aivi
Unit
Bool
Int
Float
```

Everything else below should be treated as a **standard library type** (even if an implementation chooses to represent it specially at first for performance/interop).

```aivi
Decimal
BigInt
Text
Bytes
Duration
Instant
Date
Time
TimeZone
ZonedDateTime
```

Numeric suffixes:

* `42n` → `BigInt`
* `3.14dec` → `Decimal`


## 3.2 Algebraic Data Types

### `Bool`

`Bool` has exactly two values:

```aivi
True : Bool
False : Bool
```

`if ... then ... else ...` requires a `Bool` condition, and can be understood as desugaring to a `case` on `True`/`False`.

### Creating values (“objects”)

AIVI does not have “objects” in the OO sense. You create values using:

- **Constructors** for algebraic data types (ADTs)
- **Literals** for primitives and records
- **Domain-owned literals/operators** for domain types (e.g. `2w + 3d` for `Duration`)

```aivi
Option A = None | Some A
Result E A = Err E | Ok A
```

To create ADT values, apply constructors like ordinary functions:

```aivi
someCount = Some 123
okText    = Ok "done"
bad       = Err "nope"
```

Nullary constructors (like `None`, `True`, `False`) are values.

## 3.3 Open Records (Row Polymorphism)

Records are:

* structural
* open by default

```aivi
User = { id: Int, name: Text, email: Option Text }
```

To create a record value, use a record literal:

```aivi
alice : User
alice = { id: 1, name: "Alice", email: None }
```

Record literals can spread existing records:

```aivi
alice = { ...defaults, name: "Alice" }
```

Spreads merge fields left-to-right; later entries override earlier ones.

Functions specify **minimum required fields**, not exact shapes.

```aivi
getName : { name: Text } -> Text
getName = .name
```


## 3.4 Classes and HKTs

```aivi
class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}

// Tokens explained:
// - Functor: The class name
// - F: Generic type parameter
// - *: Denotes a higher-kinded type (F takes one type argument)
// - A, B: Type variables within the definition
```

```aivi
class Monad (M *) =
  Functor (M *) & { // The & operator denotes class inheritance/aggregation
    pure: A -> M A
    flatMap: M A -> (A -> M B) -> M B
  }
```

`A & B` in type position denotes **record/type composition** (an intersection-like merge). It is primarily used for class inheritance and trait aggregation in v0.1.

Instances:

```aivi
instance Monad (Option *) = { ... }
instance E: Monad (Result E *) = { ... } // E: binds the error parameter for the Result monad instance
```


<!-- FILE: /02_syntax/04_predicates -->

# Predicates (Unified Model)

## 4.1 Predicate expressions

Any expression of type `Bool` that uses only:

* literals
* field access
* patterns
* the implicit `_`

is a **predicate expression**.

Examples:

```aivi
price > 80
_.price > 80
email == Some "x"
Some _
Ok { value } when value > 10
```

Pattern predicates like `Ok { value } when value > 10` are “match tests”: they succeed if the current value matches the pattern, and the `when` guard can refer to names bound by the pattern.

## 4.1.1 Predicate combinators

Predicate expressions support the usual boolean operators:

* `!p` (not)
* `p && q` (and, short-circuit)
* `p || q` (or, short-circuit)

These operators may appear inside any predicate position (including generator guards and patch predicates).

If you want to name predicate functions explicitly, you can treat them as ordinary functions:

```aivi
Pred A = A => Bool

andPred : Pred A -> Pred A -> Pred A
andPred p q = x => p x && q x

isActive : Pred User
isActive = .active

isPremium : Pred User
isPremium = u => u.tier == Premium

isActivePremium : Pred User
isActivePremium = andPred isActive isPremium
```


## 4.2 Implicit binding rule

Inside a predicate expression:

* `_` is bound to the **current element**
* bare field names are resolved as `_.field`
* `.field` is an accessor function (`x => x.field`), not a field value

> [!TIP]
> `filter active` is shorthand for `filter (_.active)` when `active` is a boolean field. If `active` is bound in scope, it refers to that binding instead.

```aivi
price > 80        // _.price > 80
active            // _.active
```


## 4.3 Predicate lifting

Whenever a function expects:

```aivi
A => Bool
```

a predicate expression may be supplied.

> [!NOTE]
> Predicates can also perform complex transformations by deconstructing multiple fields:
> `map { name, id } => if id > 10 then name else "no name"`

Desugaring:

```text
predicateExpr
⇒ (_ => predicateExpr)
```

Applies to:

* `filter`, `find`, `takeWhile`, `dropWhile`
* generator guards (`x -> pred`)
* patch predicates
* user-defined functions

Examples:

```aivi
users |> filter active
users |> filter (age > 18)
users |> find (email == Some "x")
xs |> takeWhile (_ < 10)
xs |> dropWhile (_ < 0)
```

```aivi
generate {
  u <- users
  u -> active && tier == Premium
  yield u
}
```

```aivi
store <| { items[price > 80].discount: 0.1 }
store <| { categories[name == "Hardware"].items[active].price: _ * 1.1 }
```

```aivi
where : (A => Bool) -> List A -> List A
where pred xs = xs |> filter pred

admins = where (role == Admin) users
activeUsers = where active users
```


## 4.4 No automatic lifting in predicates

Predicates do **not** auto-lift over `Option` or `Result`.

```aivi
filter (email == "x")      // ❌ if email : Option Text
filter (email == Some "x") // ✅
```

Reason: predicates affect **cardinality**.


<!-- FILE: /02_syntax/05_patching -->

# Patching Records

The `<|` operator applies a **declarative structural patch**. This avoids overloading `<=`, which is expected to be a normal comparison operator.

The compiler enforces that the patch shape matches the target record's type, ensuring that only existing fields are updated or new fields are added according to the record's openness. When a patch path selects a `Map` entry, the patch applies to the **value** stored at that key.

```aivi
updated = record <| { path: instruction }
```

Patching is:

* immutable
* compositional
* type-checked

Compiler checks:

* Patch paths must resolve against the target type (unknown fields/constructors are errors).
* Predicate selectors (`items[price > 80]`) must type-check as `Bool`.
* Map key selectors (`map["k"]` or `map[key == "k"]`) must use the map's key type.
* Removing fields (`-`) is only allowed when the resulting record type remains valid (e.g. not removing required fields of a closed record).


## 5.1 Path addressing

### Dot paths

```aivi
user.profile.avatar.url
```

### Traversals

```aivi
items[*]
```

### Predicates

```aivi
items[price > 80]
items[id == 1]
```

### Map key selectors

When the focused value is a `Map`, selectors address entries by key. After selection, the focus is the **value** at that key.

```aivi
settings["theme"]
usersById[key == "id-1"]
rolesById[*]
```

In map predicates, the current element is an entry record `{ key, value }`, so `key == "id-1"` is shorthand for `_.key == "id-1"`.

### Sum-type focus (prisms)

```aivi
Ok.value
Some.val
Circle.radius
```

If the constructor does not match, the value is unchanged.


## 5.2 Instructions

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert |
| `Function` | Transform existing value |
| `:= Function` | Replace with function **as data** |
| `-` | Remove field (shrinks record type) |


## 5.3 Replace / insert

```aivi
user2 = user <| {
  name: "Grace"
  profile.avatar.url: "https://img"
}
```

Intermediate records are created if missing.


## 5.4 Transform

```aivi
user3 = user <| {
  name: toUpper
  stats.loginCount: _ + 1
}
```


## 5.5 Removal

```aivi
user4 = user <| {
  email: -
  preferences.notifications.email: -
}
```

Removal is structural and reflected in the resulting type.


## 5.7 Expressive Data Manipulation

Patching allows for very concise updates to deeply nested data structures and collections.

### Deep Collection Updates
```aivi
// Update prices of all active items in a category
store2 = store <| {
  categories[name == "Hardware"].items[active].price: _ * 1
}
```

```aivi
users2 = usersById <| {
  ["id-1"].profile.name: toUpper
}
```

### Complex Sum-Type Patching
```aivi
// Move all shapes to the origin
scene2 = scene <| {
  shapes[*].Circle.center: origin
  shapes[*].Square.origin: origin
}
```

### Record Bulk Update
```aivi
// Set multiple fields based on previous state
user2 = user <| {
  name: toUpper
  status: if admin then SuperUser else Normal
  stats.lastVisit: now
}
```


<!-- FILE: /02_syntax/06_domains -->

# Domains

Domains are AIVI's mechanism for context-aware semantics. They allow the language to adapt to specific problem spaces—like time, geometry, or UI styling—by providing custom interpretations for operators, literals, and type interactions.

Instead of baking specific logic (like "days often have 24 hours but not always") into the core compiler, AIVI delegates this to **Domains**.

## Using Domains

To use a domain, you `use` it. This brings its operators and literals into scope.

```aivi
// Bring Vector math into scope
use aivi.vector

position = (10, 20)
velocity = (1, 0)

// The '+' operator now knows how to add tuples as vectors
new_pos = position + velocity
```

## Units and Deltas

Domains often introduce **Units** (measurements) and **Deltas** (changes).

### Delta Literals (Suffixes)

Deltas represent a relative change or a typed quantity. They are written as numeric literals with a suffix.

```aivi
10m      // 10 minutes (Duration) or 10 meters (Length)
30s      // 30 seconds
90deg    // 90 degrees
100px    // 100 pixels
```

These are **not** strings; they are typed values. `10m` might compile to a `Duration` struct or a float tagged as `Meters`, depending on the active domain.

```aivi
deadline = now + 10m
```



## Defining Domains

You can define your own domains to encapsulate logic. A domain relates a **Carrier Type** (the data) with **Delta Types** (changes) and **Operators**.

### Syntax

```aivi
domain Name over CarrierType = {
  // 1. Define the "change" type
  type Delta = ...

  // 2. Implement operators
  (+) : CarrierType -> Delta -> CarrierType
  (+) carrier delta = ...

  // 3. Define literals
  1d = Day 1
  ~my_sigil(...) = ...
}
```

### Example: A Simple Color Domain

```aivi
// The data
Rgb = { r: Int, g: Int, b: Int }

// The definition
domain Color over Rgb = {
  // Deltas define how values can change
  type Delta = Lightness Int | Hue Int

  // Operator: Color + Change -> Color
  (+) : Rgb -> Delta -> Rgb
  (+) color (Lightness amount) = adjust_lightness color amount
  (+) color (Hue amount)       = adjust_hue color amount

  // Define suffix literals 
  // "10l" desugars to "Lightness 10"
  1l = Lightness 1
  1h = Hue 1
}
```

### Interpretation

When you write:

```aivi
red = { r: 255, g: 0, b: 0 }
lighter = red + 10l
```

The compiler sees `red` is type `Rgb`. It looks for a domain over `Rgb` (the `Color` domain). It then desugars `10l` using the domain's rules into `Lightness 10`, and maps `+` to the domain's `(+)` function.

## Multi-Carrier Domains

Some domains cover multiple types (e.g., `Vector` over `Vec2` and `Vec3`). In v0.1, this is handled by defining the domain multiple times, once for each carrier.

```aivi
domain Vector over Vec2 = { ... }
domain Vector over Vec3 = { ... }
```


<!-- FILE: /02_syntax/07_generators -->

# Generators

Generators are **pure, pull-based sequence producers**. They are distinct from effects: a `generate { ... }` block is purely functional and cannot perform I/O.

They:

* do not perform effects
* do not suspend execution stacks
* model finite or infinite data


## 7.1 Generator type

```aivi
Generator A
```

## 7.2 Generator expressions

```aivi
gen = generate {
  yield 1
  yield 2
  yield 3
}
```

### From Python/JavaScript
Similar to `yield` syntax, but purely functional (no mutable iterator state).

### From Haskell/Scala (no list comprehension syntax)

AIVI does **not** use Haskell-style list comprehensions like:

```aivi
// Not AIVI syntax
[ x | x <- xs, p x ]
```

Instead, write the equivalent logic with a `generate` block:

```aivi
generate {
  x <- xs
  x -> p x
  yield x
}
```


## 7.3 Guards and predicates

Generators use a Scala/Haskell-style binder:

* `x <- xs` binds `x` to each element produced by `xs`
* `x = e` is a plain (pure) local binding
* `x -> pred` is a guard (filters `x`); multiple guards may appear

In a guard, `pred` is a predicate expression with the implicit `_` bound to `x` (so bare fields like `active` resolve to `x.active`).

This means these are equivalent:

```aivi
u -> isValidEmail email
u -> isValidEmail (_.email)
u -> isValidEmail u.email
```

Note: `.email` is an accessor function (`x => x.email`). It’s useful for `map .email`, but in a predicate position you usually want a value like `email` / `_.email`, not a function.

```aivi
generate {
  x <- xs
  x -> price > 80
  yield x
}
```

Predicate rules are identical to `filter`.


## 7.4 Effectful streaming (future direction)

The v0.1 surface syntax does **not** include `generate async`.

The recommended model is:

- keep `Generator` pure, and
- represent async / I/O-backed streams as an `Effect` that *produces* a generator, or via a dedicated `Stream` type in the standard library.

This aligns with `specs/OPEN_QUESTIONS.md` (“generators should be pure; use `Effect` for async pull”).
## 7.5 Expressive Sequence Logic

Generators provide a powerful, declarative way to build complex sequences without intermediate collections or mutation.

### Cartesian Products
```aivi
// Generate all pairs in a grid
grid = generate {
  x <- [0..width]
  y <- [0..height]
  yield (x, y)
}
```

### Complex Filtering and Transformation
```aivi
// Find active premium users with valid emails
processed = generate {
  u <- users
  u -> active && tier == Premium && isValidEmail email
  yield { name: u.name, email: toLower u.email }
}
```

### Expressive Infinity
```aivi
// Infinite sequence of Fibonacci numbers
fibs = generate {
  loop (a, b) = (0, 1) => {
    yield a
    recurse (b, a + b)
  }
}
```

`loop (pat) = init => { ... }` introduces a local tail-recursive loop for generators.
Inside the loop body, `recurse next` continues with the next state.


<!-- FILE: /02_syntax/08_pattern_matching -->

# Pattern Matching

## 8.1 `?` branching

```aivi
classify = v => v ?
  | 0 => "zero"
  | _ => "nonzero"
```

This is a concise way to do case analysis, similar to `match` in Rust or `case` in Haskell/Elixir.

Compiler checks:

- Non-exhaustive matches are a compile-time error unless a catch-all arm (`_`) is present.
- Unreachable arms (shadowed by earlier patterns) produce a warning.


## 8.2 Multi-clause functions

```aivi
sum =
  | []        => 0
  | [h, ...t] => h + sum t
```


## 8.3 Record Patterns

```aivi
greet =
  | { role: Admin, name } => "Welcome back, Admin {name}!"
  | { role: Guest }       => "Welcome, guest!"
  | { name }              => "Hello, {name}!"
```


## 8.4 Nested Patterns

Record patterns support dotted keys, so nested patterns can often be written without extra braces.

```aivi
processResult =
  | Ok { data.users: [first, ...] } => "First user: {first.name}"
  | Ok { data.users: [] }           => "No users found"
  | Err { code: 404 }               => "Not found"
  | Err { code, message }           => "Error {code}: {message}"
```


## 8.5 Guards

Patterns can have guards using `when`:

```aivi
classify =
  | n when n < 0   => "negative"
  | 0              => "zero"
  | n when n < 10  => "small"
  | n when n < 100 => "medium"
  | _              => "large"
```


## 8.6 Usage Examples

### Option Handling

```aivi
Option A = None | Some A

getOrDefault = default => v => v ?
  | None       => default
  | Some value => value

userName = user.nickname |> getOrDefault "Anonymous"
```

### Result Processing

```aivi
Result E A = Err E | Ok A

handleResult =
  | Ok data => processData data
  | Err e => logError e

// With chaining
fetchUser id
  |> handleResult
  |> renderView
```

### List Processing

```aivi
// Safe head
head =
  | [] => None
  | [x, ...] => Some x

// Take first n
take = (n, xs) => (n, xs) ?
  | (n, _) when n <= 0 => []
  | (_, [])            => []
  | (n, [x, ...xs])    => [x, ...take (n - 1, xs)]

// Zip two lists
zip =
  | ([], _) => []
  | (_, []) => []
  | ([x, ...xs], [y, ...ys]) => [(x, y), ...zip (xs, ys)]
```

### Tree Traversal

```aivi
Tree A = Leaf A | Node (Tree A) (Tree A)

depth =
  | Leaf _ => 1
  | Node left right => 1 + max (depth left) (depth right)

flatten =
  | Leaf x => [x]
  | Node left right => flatten left ++ flatten right
```

### Expression Evaluation

```aivi
Expr = Num Int | Add Expr Expr | Mul Expr Expr

eval =
  | Num n   => n
  | Add a b => eval a + eval b
  | Mul a b => eval a * eval b

// (2 + 3) * 4 = 20
expr = Mul (Add (Num 2) (Num 3)) (Num 4)
result = eval expr
```
## 8.7 Expressive Pattern Orchestration

Pattern matching excels at simplifying complex conditional branches into readable declarations.

```aivi
headerLabel = response ?
  | { data.user.profile@{ name } } => name
  | { data.guest: True }           => "Guest"
  | _                              => "Unknown"
```

### Concise State Machines
```aivi
// Update application state based on event
nextState = (state, event) => (state, event) ?
  | (Idle, Start)    => Running
  | (Running, Pause) => Paused
  | (Paused, Resume) => Running
  | (Running, Stop)  => Idle
  | _                => state // Unchanged on invalid events
```

### Expressive Logic Branches
```aivi
// Business rule mapping
discount = user => user ?
  | _ when user.age > 65 && user.tier == Gold => 0.3
  | _ when user.tier == Gold                  => 0.2
  | _ when user.tier == Silver                => 0.1
  | _                                         => 0.0
```


<!-- FILE: /02_syntax/09_effects -->

# Effects

## 9.1 The `Effect E A` Type

Effectful operations in AIVI are modeled using the `Effect E A` type, where:
- `E` is the **error domain** (describing what could go wrong).
- `A` is the **successful return value**.

### Semantics
- **Atomic Progress**: Effects are either successfully completed, failed with `E`, or **cancelled**.
- **Cancellation**: Cancellation is an asynchronous signal that stops the execution of an effect. When cancelled, the effect is guaranteed to run all registered cleanup (see [Resources](15_resources.md)).
- **Transparent Errors**: Errors in `E` are part of the type signature, forcing explicit handling or propagation.

### Core operations (surface names)

Effect sequencing is expressed via `effect { ... }` blocks, but the underlying interface is:

- `pure : A -> Effect E A` (return a value)
- `bind : Effect E A -> (A -> Effect E B) -> Effect E B` (sequence)
- `fail : E -> Effect E A` (abort with an error)

For *handling* an effect error as a value, the standard library provides:

- `attempt : Effect E A -> Effect E (Result E A)`

### Examples (core operations)

`pure` lifts a value into an effect:

```aivi
pureExample : Effect Text Int
pureExample =
  pure 42
```

`bind` sequences effects explicitly (the `effect { ... }` block desugars to `bind`):

```aivi
bindExample : Effect Text Int
bindExample =
  (pure 41 |> bind) (n => pure (n + 1))
```

`fail` aborts an effect with an error value:

```aivi
failExample : Effect Text Int
failExample =
  fail "boom"
```

`attempt` runs an effect and captures success/failure as a `Result`:

```aivi
attemptExample : Effect Text Text
attemptExample = effect {
  res <- attempt (fail "nope")
  res ?
    | Ok _  => pure "unexpected"
    | Err e => pure e
}
```

### `load`

The standard library function `load` lifts a typed `Source` (see [External Sources](12_external_sources.md)) into an `Effect`.

```aivi
load : Source K A -> Effect (SourceError K) A
```

## 9.2 `effect` blocks

```aivi
main = effect {
  cfg <- load (file.json "config.json")
  _ <- print "loaded"
  pure Unit
}
```

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside an `effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding
- `x <- res` acquires a `Resource` (see [Resources](15_resources.md))
- Branching is done with ordinary expressions (`if`, `case`, `?`); `->` guards are generator-only.
- If a final expression is present, it must be an `Effect` (commonly `pure value` or an effect call like `print "..."`).
- If there is no final expression, the block defaults to `pure Unit`.

Compiler checks:

- Expression statements must be `Effect`-typed.
- Discarding an `Effect` result is allowed with a bare expression statement; binding to `_` is optional.

### `if` with nested blocks inside `effect`

`if` is an expression, so you can branch inside an `effect { … }` block. When a branch needs multiple steps, use a nested `effect { … }` block (since `{ … }` is reserved for record-shaped forms).

This pattern is common when a branch needs multiple effectful steps:

```aivi
main = effect {
  u <- loadUser
  token <- if u.isAdmin then effect {
    _ <- log "admin login"
    token <- mintToken u
    pure token
  } else pure "guest"
  pure token
}
```

Desugaring-wise, the `if … then … else …` appears inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `effect { … }` expressions inside `if`

An explicit `effect { … }` is itself an expression of type `Effect E A`. If you write `effect { … }` in an `if` branch, you usually want to run (bind) the chosen effect:

```aivi
main = effect {
  token <- if shouldMint then mintToken user else pure "guest"
  pure token
}
```

If you instead write `if … then effect { … } else effect { … }` *without* binding it, the result of the `if` is an `Effect …` value, not a sequence of steps in the surrounding block (unless it is the final expression of that surrounding `effect { … }`).


## 9.3 Effects and patching

```aivi
authorize = user => user <| {
  roles: _ ++ ["Admin"]
  lastLogin: now
}
```

Patches are pure values. Apply them where you have the record value available (often inside an `effect` block after decoding/loading).


## 9.4 Comparison and Translation

The `effect` block is the primary way to sequence impure operations. It translates directly to monadic binds.

| `effect` Syntax | Explicit Monadic Syntax |
| :--- | :--- |
| `val = effect { x <- f; g x }` | `val = (f |> bind) (x => g x)` |
| `effect { f; g }` | `(f |> bind) (_ => g)` |

Example translation:

```aivi
// Sequence with effect block
transfer fromAccount toAccount amount = effect {
  balance <- getBalance fromAccount
  if balance >= amount then effect {
    _ <- withdraw fromAccount amount
    _ <- deposit toAccount amount
    pure Unit
  } else fail InsufficientFunds
}

// Equivalent functional composition
transfer fromAccount toAccount amount =
  (getBalance fromAccount |> bind) (balance =>
    if balance >= amount then
      (withdraw fromAccount amount |> bind) (_ =>
        (deposit toAccount amount |> bind) (_ =>
          pure Unit))
    else
      fail InsufficientFunds
  )
```
## 9.5 Expressive Effect Composition

Effect blocks can be combined with pipelines and pattern matching to create very readable business logic.

### Concatenating effectful operations
```aivi
// Fetch config, then fetch data, then log
setup = effect {
  cfg <- loadConfig "prod.json"
  data <- fetchRemoteData cfg
  _ <- logSuccess data
  pure Unit
}
```

### Expressive Error Handling
```aivi
// Attempt operation, providing a typed default on error
getUser = id => effect {
  res <- attempt (api.fetchUser id)
  res ?
    | Ok user => pure user
    | Err _   => pure GuestUser
}

validatedUser = effect {
  u <- getUser 123
  if u.age > 18 then pure (toAdmin u) else fail TooYoung
}
```


<!-- FILE: /02_syntax/10_modules -->

# Modules

## 10.1 Module Definitions

Modules are the primary unit of code organization, encapsulation, and reuse in AIVI. They define a closed scope and explicitly export symbols for public use.

```aivi
module my.utility.math = {
  export add, subtract
  export pi
  
  pi = 3.14159
  add = a b => a + b
  subtract = a b => a - b
  
  // Internal helper, not exported
  abs = n => if n < 0 then -n else n
}
```


## 10.2 Module Pathing (Dot Separator)

Modules are identified by hierarchical paths using common **dot notation**. This separates logical namespaces. By convention:
- `aivi.*` — Standard library
- `vendor.name.*` — Foreign libraries
- `user.app.*` — Application-specific logic

Module resolution is static and determined at compile time based on the project manifest.


## 10.3 Importing and Scope

Use the `use` keyword to bring symbols from another module into the current scope.

### Basic Import
```aivi
use aivi
```

### Selective / Selective Hiding
```aivi
use aivi.calendar (Date, isLeapYear)
use aivi.list hiding (map, filter)
```

### Renaming / Aliasing
```aivi
use aivi.calendar as Cal
use vendor.legacy.math (v1_add as add)
```

Compiler checks:

- Importing a missing module or symbol is a compile-time error.
- Unused imports produce a warning (suppressed if importing solely for a domain side-effect in v0.1).


## 10.4 Domain Exports

Modules are the primary vehicle for delivering **Domains**. Exporting a domain automatically exports its carrier type, delta types, and operators.

```aivi
module geo.vector = {
  export domain Vector
  export Vec2
  
  Vec2 = { x: Float, y: Float }
  
  domain Vector over Vec2 = {
    (+) : Vec2 -> Vec2 -> Vec2
    (+) a b = { x: a.x + b.x, y: a.y + b.y }
  }
}
```

When another module calls `use geo.vector`, it gains the ability to use `+` on `Vec2` records.


## 10.5 First-Class Modules

Modules are statically resolved but behave like first-class records within the compiler's intermediate representation. This enables powerful composition patterns.

### Nested Modules
```aivi
module aivi = {
  module calendar = { ... }
  module number = { ... }
}
```

### Module Re-exports
A module can aggregate other modules, acting as a facade.

```aivi
module aivi.prelude = {
  export domain Calendar, Color
  export List, Result, Ok, Err
  
  use aivi.calendar (domain Calendar)
  use aivi.color (domain Color)
  use aivi (List, Result, Ok, Err)
}
```


## 10.6 The Prelude

Every AIVI module implicitly starts with `use aivi.prelude`. This provides access to the core language types and the most common domains without boilerplate.

To opt-out of this behavior (mandatory for the core stdlib itself):

```aivi
@no_prelude
module aivi.bootstrap = {
  // Pure bootstrap logic
}
```


## 10.7 Circular Dependencies

Circular module dependencies are **strictly prohibited** at the import level. The compiler enforces a Directed Acyclic Graph (DAG) for module resolution. For mutually recursive types or functions, they must reside within the same module or be decoupled via higher-order abstractions.
## 10.8 Expressive Module Orchestration

Modules allow for building clean, layered architectures where complex internal implementations are hidden behind simple, expressive facades.

### Clean App Facade
```aivi
// Aggregate multiple sub-modules into a single clean API
module my.app.api = {
  export login, fetchDashboard, updateProfile
  
  use my.app.auth (login)
  use my.app.data (fetchDashboard)
  use my.app.user (updateProfile)
}
```

### Domain Extension Pattern
```aivi
// Enhance an existing domain with local helpers
module my.geo.utils = {
  export domain Vector
  export distanceToOrigin, isZero
  
  use geo.vector (domain Vector, Vec2)
  
  distanceToOrigin = v => sqrt (v.x * v.x + v.y * v.y)
  isZero = v => v.x == 0 && v.y == 0
}
```

### Context-Specific Environments (Static Injection)

This pattern allows you to **statically swap** entire module implementations for different build contexts (e.g., Test vs. Prod). This is not for runtime configuration (see below), but for compile-time substitution of logic.

```aivi
// 1. Define the production module
module my.app.api = {
  export fetchDashboard
  fetchDashboard = ... // Real HTTP call
}

// 2. Define the test module (same interface, different logic)
module my.app.api.test = {
  export fetchDashboard
  fetchDashboard = _ => { id: 1, title: "Mock Dash" }
}
```

To use the test environment, your test entry point (`tests/main.aivi`) simply imports the test module instead of the production one:

```aivi
// within tests/main.aivi
use my.app.api.test (fetchDashboard) // injected mock
```

## 10.9 Runtime Configuration (Env Vars)

For values that change between deployments (like API URLs or DB passwords) without changing code, use **Runtime Configuration** via the `Env` source.

Do not use module swapping for this. Instead, inject the configuration as data.

See [12.4 Environment Sources](12_external_sources.md#124-environment-sources-env) for details.

```aivi
// Instead of hardcoding, load from environment
config : Source Env { apiUrl: Text }
config = env.decode { apiUrl: "https://localhost:8080" }

connect = effect {
  cfg <- load config
  // ... use cfg.apiUrl
}
```


<!-- FILE: /02_syntax/13_sigils -->

# Sigils

Sigils provide custom parsing for complex literals. They start with `~` followed by a tag and a delimiter.

```aivi
// Regex
pattern = ~r/\w+@\w+\.\w+/

// URL
endpoint = ~u(https://api.example.com)

// Date
birthday = ~d(1990-12-31)
```

Domains define these sigils to validate and construct types at compile time.

## Structured sigils

Some domains parse sigils as **AIVI expressions** rather than raw text. For v1.0, the `Collections` domain defines:

```aivi
// Map literal (entries use =>, spread with ...)
users = ~map{
  "id-1" => { name: "Alice" }
  "id-2" => { name: "Bob" }
}

// Set literal (spread with ...)
tags = ~set[...baseTags, "hot", "new"]
```

The exact meaning of a sigil is domain-defined; see [Collections](../05_stdlib/00_core/28_collections.md) for `~map` and `~set`.


<!-- FILE: /02_syntax/12_external_sources -->

# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with full type safety enforced during decoding.

## 12.1 The Source Type

```aivi
Source K A
```

- `K` — the **kind** of source (File, Http, Db, etc.)
- `A` — the **decoded type** of the content

Sources are effectful. Loading a source performs I/O and returns an `Effect E A` (where `E` captures the possible source errors). All source interactions must occur within an `effect` block.

Typical API shape:

```aivi
load : Source K A -> Effect (SourceError K) A
```

To handle errors as values, use `attempt` (see [Effects](09_effects.md)):

```aivi
effect {
  res <- attempt (load (file.read "./README.md"))
  res ?
    | Ok txt => pure txt
    | Err _  => pure "(missing)"
}
```


## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

```aivi
// Read entire file as text
readme = file.read "./README.md"

// Stream bytes from a large file
blob = file.stream "./large.bin"

// Read structured CSV (the expected type drives decoding)
User = { id: Int, name: Text, email: Text }
users : Source File (List User)
users = file.csv "./users.csv"
```


## 12.3 HTTP Sources

Typed REST/API integration.

```aivi
User = { id: Int, name: Text }

// Typed GET request (inferred type)
users = http.get ~u(https://api.example.com/v1/users)

// Request with headers and body
req = http.request {
  method: Post
  url: ~u(https://api.example.com/v1/users)
  headers: [("Content-Type", "application/json")]
  body: Some (Json.encode { name: "New User" })
}
```


## 12.4 Environment Sources (Env)

Typed access to environment configuration. Values are decoded using the expected type and optional defaults.

```aivi
// Read a single environment variable as Text
appEnv : Source Env Text
appEnv = env.get "APP_ENV"

// Decode a typed configuration record with defaults
DbConfig = {
  driver: Text
  url: Text?
  host: Text?
  port: Int?
  user: Text?
  password: Text?
  database: Text?
}

defaultDbConfig : DbConfig
defaultDbConfig = {
  driver: "sqlite"
  url: Some "./local.db"
  host: None
  port: None
  user: None
  password: None
  database: None
}

dbConfig : Source Env DbConfig
dbConfig = env.decode defaultDbConfig
```

## 12.5 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

```aivi
// SQLite connection
db = sqlite.open "./local.db"

User = { id: Int, name: Text }

// Typed query source (the expected type drives decoding)
activeUsers : Source Db (List User)
activeUsers = db.query "SELECT id, name FROM users WHERE active = 1"
```

See the [Database Domain](../05_stdlib/03_system/23_database.md) for table operations, deltas, and migrations.


## 12.6 Email Sources

Interacting with mail servers (IMAP/SMTP).

```aivi
// Fetch unread emails
inbox = email.imap {
  host: "imap.gmail.com"
  filter: "UNSEEN"
}

// Sending as a sink effect
sendWelcome = user => email.send {
  to: user.email
  subject: "Welcome!"
  body: "Glad to have you, {user.name}"
}
```


## 12.7 LLM Sources

AIVI treats Large Language Models as typed probabilistic sources. This is a core part of the AIVI vision for intelligent data pipelines.

```aivi
// Define expected output shape
Sentiment = Positive | Negative | Neutral

Analysis = {
  sentiment: Sentiment
  summary: Text
}

// LLM completion with strict schema enforcement
analyze input = llm.complete {
  model: "gpt-4o"
  prompt: "Analyze this feedback: {input}"
  schema: Analysis
}
```


## 12.8 Image Sources

Images are typed by their metadata and pixel data format.

```aivi
Image A = { width: Int, height: Int, format: ImageFormat, pixels: A }

// Load image metadata only
meta = file.imageMeta "./photo.jpg"

// Load full image with RGB pixel access
photo = file.image "./photo.jpg"
```


## 12.9 S3 / Cloud Storage Sources

Integration with object storage.

```aivi
// Bucket listings
images = s3.bucket "my-assets" |> s3.list "thumbnails/"

// Fetch object content
logo = s3.get "my-assets" "branding/logo.png"
```

> [!NOTE]
> Browser sources are part of the AIVI long-term vision for end-to-end automation but are considered **Experimental** and may not be fully available in the initial WASM-targeted phase.


## 12.10 Compile-Time Sources (@static)

Some sources are resolved at compile time and embedded into the binary. This ensures zero latency/failure at runtime.

```aivi
@static
version = file.read "./VERSION"

@static
locales = file.json "./i18n/en.json"
```


<!-- FILE: /02_syntax/14_decorators -->

# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy (Constraints)

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Only the standard decorators listed here are allowed in v0.1. Unknown decorators are a compile error.
- User-defined decorators are not supported in v0.1.

## 14.1 Syntax

```aivi
@decorator_name
@decorator_name value
@decorator_name { key: value }
```

Decorators appear before the binding they annotate.


## 14.2 Standard Decorators

### Compile-Time

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@static` | `@static x = file.read "..."` | Embed at compile time |
| `@inline` | `@inline f = ...` | Always inline function |
| `@deprecated` | `@deprecated msg` | Emit warning on use |

### Tooling (MCP)

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@mcp_tool` | `@mcp_tool fetchData = ...` | Expose as MCP tool |
| `@mcp_resource` | `@mcp_resource config = ...` | Expose as MCP resource |

### Testing

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@test` | `@test add_is_commutative = ...` | Mark a definition as a test case |

### Pragmas (Module-level)
| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@no_prelude` | `@no_prelude module M = ...` | Skip implicit prelude import |
## 14.3 Decorator Desugaring

Decorators desugar to compile-time metadata:

| Surface | Desugared |
| :--- | :--- |
| `@static x = file.read ...` | Compile-time evaluation |
| `@mcp_tool f = ...` | Register in MCP manifest |


## 14.4 Usage Examples

### Compile-Time Embedding

```aivi
@static
version : Text
version = file.read "./VERSION"

@static
schema : JsonSchema
schema = file.json "./schema.json"
```

### MCP Tools

```aivi
@mcp_tool
searchDocs : Query -> Effect Http (List Document)
searchDocs query = http.get "https://api.example.com/search?q={query}"

@mcp_resource
appConfig : Source File Config
appConfig = file.json "./config.json"
```


<!-- FILE: /02_syntax/15_resources -->

# Resource Management

AIVI provides a dedicated `Resource` type to manage lifecycles (setup and teardown) in a declarative way. This ensures that resources like files, sockets, and database connections are always reliably released, even in the event of errors or task cancellation.


## 15.1 Defining Resources

Resources are defined using `resource` blocks. The syntax is analogous to generators: you perform setup, `yield` the resource, and then perform cleanup.

The code after `yield` is guaranteed to run when the resource goes out of scope.

```aivi
// Define a reusable resource
managedFile path = resource {
  handle <- file.open path  // Acquire
  yield handle              // Provide to user
  _ <- file.close handle    // Release
}
```

This declarative approach hides the complexity of error handling and cancellation checks.


## 15.2 Using Resources

Inside an `effect` block, you use the `<-` binder to acquire a resource. This is similar to the generator binder, but instead of iterating, it scopes the resource to the current block.

```aivi
main = effect {
  // Acquire resource
  f <- managedFile "data.txt"
  
  // Use resource
  content <- file.readAll f
  _ <- print content
  pure Unit
} // f is automatically closed here
```

### Multiple Resources

You can acquire multiple resources in sequence. They will be released in reverse order of acquisition (LIFO).

```aivi
copy src dest = effect {
  input  <- managedFile src
  output <- managedFile dest
  
  _ <- file.copyTo input output
  pure Unit
}
```



<!-- FILE: /02_syntax/00_grammar -->

# Concrete Syntax (EBNF draft)

This chapter is a **draft concrete grammar** for the surface language described in the Syntax section. It exists to make parsing decisions explicit and to highlight places where the compiler should emit helpful diagnostics.

This chapter is intentionally pragmatic: it aims to be complete enough to build a real lexer/parser/LSP for the current spec and repo examples, even though many parts of the language are still evolving.


## 0.1 Lexical notes

> These are **normative** for parsing. Typing/elaboration rules live elsewhere.

### Whitespace and comments

- Whitespace separates tokens and is otherwise insignificant (no indentation sensitivity in v0.1).
- Line comments start with `//` and run to the end of the line.
- Block comments start with `/*` and end with `*/` (nesting is not required).

### Identifiers

- `lowerIdent` starts with a lowercase ASCII letter: values, functions, fields.
- `UpperIdent` starts with an uppercase ASCII letter: types, constructors, modules, domains, classes.
- After the first character, identifiers may contain ASCII letters, digits, and `_`.
- Keywords are reserved and cannot be used as identifiers.

### Keywords (v0.1)

```text
as do domain effect else export generate hiding if
instance module over recurse resource then type use yield loop
```

(`True`, `False`, `None`, `Some`, `Ok`, `Err` are ordinary constructors, not keywords.)

### Literals (minimal set for v0.1)

- `IntLit`: decimal digits (e.g. `0`, `42`).
- `FloatLit`: digits with a fractional part (e.g. `3.14`).
- `TextLit`: double-quoted with escapes and interpolation (see below).
- `CharLit`: single-quoted (optional in v0.1; many examples can use `Text` instead).
- `IsoInstantLit`: ISO-8601 instant-like token (e.g. `2024-05-21T12:00:00Z`), used by the `Calendar`/`Time` domains.
- `SuffixedNumberLit`: `IntLit` or `FloatLit` followed immediately by a suffix (e.g. `10px`, `100%`, `30s`, `1min`).

`SuffixedNumberLit` is *lexical*; its meaning is **domain-resolved** (see Domains). The lexer does not decide whether `1m` is “month” or “meter”.

### Text literals and interpolation

Text literals are delimited by `"` and support interpolation segments `{ Expr }`:

```aivi
"Hello"
"Count: {n}"
"{user.name}: {status}"
```

Inside a `TextLit`, `{` starts interpolation and `}` ends it; braces must be balanced within the interpolated expression.

### Separators (layout)

Many constructs accept either:
- one or more newlines, or
- `;`

as a separator. The parser should treat consecutive separators as one.

In addition, many comma-delimited forms allow `,` as an alternative separator.

We name these separators in the grammar:

```ebnf
Sep        := ( Newline | ";" ) { ( Newline | ";" ) } ;
FieldSep   := Sep | "," ;
```

### Ellipsis

- `...` is a single token (ellipsis) used for list rest patterns and spread entries.


## 0.2 Top level

```ebnf
Program        := { TopItem } ;
TopItem        := { Decorator } (ModuleDef | Definition) ;

Decorator      := "@" lowerIdent [ DecoratorArg ] Sep ;
DecoratorArg   := Expr | RecordLit ;

Definition     := ValueSig
               | ValueBinding
               | TypeAlias
               | TypeDef
               | DomainDef
               | ClassDef
               | InstanceDef ;

ValueSig       := lowerIdent ":" Type Sep ;
ValueBinding   := Pattern "=" Expr Sep ;

TypeAlias      := "type" UpperIdent [ TypeParams ] "=" TypeRhs Sep ;
TypeDef        := UpperIdent [ TypeParams ] "=" TypeRhs Sep ;
TypeParams     := UpperIdent { UpperIdent } ;
TypeRhs        := Type
               | RecordType
               | [ Sep? "|" ] ConDef { Sep? "|" ConDef } ;
ConDef         := UpperIdent { TypeAtom } ;

ModuleDef      := "module" ModulePath "=" ModuleBody Sep ;
ModulePath     := ModuleSeg { "." ModuleSeg } ;
ModuleSeg      := lowerIdent | UpperIdent ;
ModuleBody     := "{" { ModuleItem } "}" ;
ModuleItem     := ExportStmt | UseStmt | Definition | ModuleDef ;
ExportStmt     := "export" ( "*" | ExportList ) Sep ;
ExportList     := ExportItem { "," ExportItem } ;
ExportItem     := lowerIdent | UpperIdent | ("domain" UpperIdent) ;
UseStmt        := "use" ModulePath [ UseSpec ] Sep ;
UseSpec        := "as" UpperIdent
               | "(" ImportList ")"
               | "hiding" "(" ImportList ")" ;
ImportList     := ImportItem { "," ImportItem } ;
ImportItem     := (lowerIdent | UpperIdent | ("domain" UpperIdent)) [ "as" (lowerIdent | UpperIdent) ] ;

DomainDef      := "domain" UpperIdent "over" Type "=" "{" { DomainItem } "}" Sep ;
DomainItem     := TypeAlias | TypeDef | ValueSig | ValueBinding | OpDef | DeltaLitBinding ;
OpDef          := "(" Operator ")" ":" Type Sep
               | "(" Operator ")" Pattern { Pattern } "=" Expr Sep ;
Operator       := "+" | "-" | "*" | "/" | "%" | "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" | "++" | "??"
               | "&" | "|" | "^" | "~" | "<<" | ">>" ;
DeltaLitBinding:= SuffixedNumberLit "=" Expr Sep ;

ClassDef       := "class" UpperIdent ClassParams "=" Type Sep ;
ClassParams    := ClassParam { ClassParam } ;
ClassParam     := UpperIdent
               | "(" UpperIdent "*" { "*" } ")" ;

InstanceDef    := "instance" [ UpperIdent ":" ] UpperIdent InstanceHead "=" RecordLit Sep ;
InstanceHead   := "(" Type ")" ;
```


## 0.3 Expressions

```ebnf
Expr           := IfExpr ;

IfExpr         := "if" Expr "then" Expr "else" Expr
               | LambdaExpr ;

LambdaExpr     := LambdaArgs "=>" Expr
               | MatchExpr ;
LambdaArgs     := PatParam { PatParam } ;
PatParam       := lowerIdent
               | "_"
               | RecordPat
               | TuplePat
               | ListPat
               | "(" PatParam ")" ;

MatchExpr      := PipeExpr [ "?" MatchArms ] ;
MatchArms      := Sep? "|" Arm { Sep "|" Arm } ;
Arm            := Pattern [ "when" Expr ] "=>" Expr ;

PipeExpr       := CoalesceExpr { "|>" CoalesceExpr } ;

CoalesceExpr   := OrExpr { "??" OrExpr } ;
OrExpr         := AndExpr { "||" AndExpr } ;
AndExpr        := EqExpr { "&&" EqExpr } ;
EqExpr         := CmpExpr { ("==" | "!=") CmpExpr } ;
CmpExpr        := BitOrExpr { ("<" | "<=" | ">" | ">=") BitOrExpr } ;
BitOrExpr      := BitXorExpr { "|" BitXorExpr } ;
BitXorExpr     := BitAndExpr { "^" BitAndExpr } ;
BitAndExpr     := ShiftExpr { "&" ShiftExpr } ;
ShiftExpr      := AddExpr { ("<<" | ">>") AddExpr } ;
AddExpr        := MulExpr { ("+" | "-" | "++") MulExpr } ;
MulExpr        := UnaryExpr { ("*" | "/" | "%") UnaryExpr } ;
UnaryExpr      := ("!" | "-" | "~" ) UnaryExpr
               | PatchExpr ;

PatchExpr      := AppExpr { "<|" PatchLit } ;

AppExpr        := PostfixExpr { PostfixExpr } ;
PostfixExpr    := Atom { "." lowerIdent } ;

Atom           := Literal
               | lowerIdent
               | UpperIdent
               | "." lowerIdent                 (* accessor sugar *)
               | "(" Expr ")"
               | TupleLit
               | ListLit
               | RecordLit
               | Block
               | EffectBlock
               | GenerateBlock
               | ResourceBlock
               ;

Block          := "do" "{" { Stmt } "}" ;
EffectBlock    := "effect" "{" { Stmt } "}" ;
GenerateBlock  := "generate" "{" { GenStmt } "}" ;
ResourceBlock  := "resource" "{" { ResStmt } "}" ;

Stmt           := BindStmt | ValueBinding | Expr Sep ;
BindStmt       := Pattern "<-" Expr Sep ;

GenStmt        := BindStmt
               | GuardStmt
               | ValueBinding
               | "yield" Expr Sep
               | "loop" Pattern "=" Expr "=>" "{" { GenStmt } "}" Sep ;
GuardStmt      := lowerIdent "->" Expr Sep ;

ResStmt        := ValueBinding
               | BindStmt
               | Expr Sep
               | "yield" Expr Sep ;

TupleLit       := "(" Expr "," Expr { "," Expr } ")" ;
ListLit        := "[" [ Expr { FieldSep Expr } | Range ] "]" ;
Range          := Expr ".." Expr ;

RecordLit      := "{" { RecordEntry } "}" ;
RecordEntry    := RecordField | RecordSpread ;
RecordField    := lowerIdent [ ":" Expr ] [ FieldSep ] ;
RecordSpread   := "..." Expr [ FieldSep ] ;

MapLit         := "~map" "{" [ MapEntry { FieldSep MapEntry } ] "}" ;
SetLit         := "~set" "[" [ SetEntry { FieldSep SetEntry } ] "]" ;
MapEntry       := Spread | Expr "=>" Expr ;
SetEntry       := Spread | Expr ;
Spread         := "..." Expr ;

SigilLit       := MapLit | SetLit | RawSigilLit ;
RawSigilLit    := "~" lowerIdent SigilBody ;
SigilBody      := SigilParen | SigilBracket | SigilBrace | SigilRegex ;
SigilParen     := "(" SigilText ")" ;
SigilBracket   := "[" SigilText "]" ;
SigilBrace     := "{" SigilText "}" ;
SigilRegex     := "/" SigilRegexText "/" [ lowerIdent ] ;

Literal        := "True"
               | "False"
               | IntLit
               | FloatLit
               | TextLit
               | CharLit
               | IsoInstantLit
               | SuffixedNumberLit
               | SigilLit ;
```

**Notes**

- `{ ... }` is reserved for record-shaped forms (`RecordLit`, `RecordType`, `RecordPat`, `PatchLit`, and module/domain bodies).
- Multi-statement expression blocks use `do { ... }`, so the parser never needs to guess whether `{ ... }` is a record literal or a block.
- `.field` is shorthand for `x => x.field` (a unary accessor function).
- `_` is *not* a value. It only appears in expressions as part of the placeholder-lambda sugar (see `specs/04_desugaring/02_functions.md`).
- `RawSigilLit` content (`SigilText` / `SigilRegexText`) is lexed as raw text until the matching delimiter; `~map{}` and `~set[]` are parsed as structured literals (`MapLit` / `SetLit`).
- `RecordSpread` (`...expr`) merges fields left-to-right; later fields override earlier ones.


## 0.4 Patching

```ebnf
PatchLit       := "{" { PatchEntry } "}" ;
PatchEntry     := Path ":" PatchInstr [ FieldSep ] ;
PatchInstr     := "-" | ":=" Expr | Expr ;

Path           := PathSeg { [ "." ] PathSeg } ;
PathSeg        := lowerIdent
               | UpperIdent "." lowerIdent
               | Select ;
Select         := "[" ( "*" | Expr ) "]" ;
```

**Notes**

- `PathSeg` is intentionally permissive in this draft: patch paths, traversal selectors, and prism-like focuses share syntax.
- A compiler should reject ill-typed or ill-scoped path forms with a targeted error (e.g. “predicate selector expects a `Bool` predicate”).


## 0.5 Multi-clause unary functions

A *unary* multi-clause function can be written using arms directly:

```ebnf
ValueBinding   := lowerIdent "=" FunArms Sep ;
FunArms        := "|" Arm { Sep "|" Arm } ;
```

This form desugars to a single-argument function that performs a `case` on its input (see `specs/04_desugaring/04_patterns.md`).

If you want multi-argument matching, match on a tuple:

```aivi
nextState =
  | (Idle, Start) => Running
  | (state, _)    => state
```

## 0.6 Types

```ebnf
Type           := TypeArrow ;
TypeArrow      := TypeAnd [ "->" TypeArrow ] ;
TypeAnd        := TypeApp { "&" TypeApp } ;
TypeApp        := TypeAtom { TypeAtom } ;
TypeAtom       := UpperIdent
               | lowerIdent
               | "*"
               | "(" Type ")"
               | TupleType
               | RecordType ;

TupleType      := "(" Type "," Type { "," Type } ")" ;
RecordType     := "{" { RecordTypeField } "}" ;
RecordTypeField:= lowerIdent ":" Type { FieldDecorator } [ FieldSep ] ;
FieldDecorator := "@" lowerIdent [ DecoratorArg ] ;
```

## 0.7 Patterns

```ebnf
Pattern        := PatAtom [ "@" Pattern ] ;
PatAtom        := "_"
               | lowerIdent
               | UpperIdent
               | Literal
               | TuplePat
               | ListPat
               | RecordPat
               | ConPat ;

ConPat         := UpperIdent { PatAtom } ;
TuplePat       := "(" Pattern "," Pattern { "," Pattern } ")" ;
ListPat        := "[" [ Pattern { "," Pattern } [ "," "..." [ (lowerIdent | "_") ] ] ] "]" ;

RecordPat      := "{" { RecordPatField } "}" ;
RecordPatField := RecordPatKey [ (":" Pattern) | ("@" Pattern) ] [ FieldSep ] ;
RecordPatKey   := lowerIdent { "." lowerIdent } ;
```


## 0.9 Diagnostics (where the compiler should nag)

- **Likely-missed `do`**: if `{ ... }` contains `=` bindings or statement separators, error and suggest `do { ... }` (since `{ ... }` is record-shaped).
- **Arms without a `?`**: `| p => e` is only valid after `?` *or* directly after `=` in the multi-clause unary function form.
- **`_` placeholder**: `_ + 1` is only legal where a unary function is expected; otherwise error and suggest `x => x + 1`.
- **Deep keys in record literals**: `a.b: 1` should be rejected in record literals (suggest patching with `<|` if the intent was a path).


<!-- FILE: /05_stdlib/00_core/01_prelude -->

# Standard Library: Prelude

The **Prelude** is your default toolkit. It acts as the "standard library of the standard library," automatically using the core types and domains you use in almost every program (like `Int`, `List`, `Text`, and `Result`). It ensures you don't have to write fifty `use` lines just to add two numbers or print "Hello World".

```aivi
module aivi.prelude = {
  // Core types
  export Int, Float, Bool, Text, Char, Bytes
  export List, Option, Result, Tuple
  
  // Standard domains
  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector
  
  // Re-exports
  use aivi
  use aivi.text
  use aivi.calendar
  use aivi.duration
  use aivi.color
  use aivi.vector
}
```

## Opting Out

```aivi
@no_prelude
module my.custom.module = {
  // Nothing used automatically
  use aivi (Int, Bool)
}
```

## Rationale

- Common domains (dates, colors, vectors) are used universally
- Delta literals should "just work" without explicit `use`
- Explicit opt-out preserves control for advanced use cases


<!-- FILE: /05_stdlib/00_core/02_text -->

# Text Module

The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
It focuses on predictable, Unicode-aware behavior, and uses `Option`/`Result` instead of
sentinel values like `-1`.

## Overview

```aivi
use aivi.text

greeting = "Hello, AIVI!"

len = length greeting
words = split " " greeting
upper = toUpper greeting
```

## Types

```aivi
type Bytes
type Encoding = Utf8 | Utf16 | Utf32 | Latin1
type TextError = InvalidEncoding Encoding
```

## Core API (v0.1)

### Length and inspection

| Function | Explanation |
| --- | --- |
| **length** text<br><pre><code>`Text -> Int`</code></pre> | Returns the number of Unicode scalar values in `text`. |
| **isEmpty** text<br><pre><code>`Text -> Bool`</code></pre> | Returns `true` when `text` has zero length. |

### Character predicates

| Function | Explanation |
| --- | --- |
| **isDigit** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode digit. |
| **isAlpha** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode letter. |
| **isAlnum** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode letter or digit. |
| **isSpace** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode whitespace. |
| **isUpper** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is uppercase. |
| **isLower** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is lowercase. |

### Search and comparison

| Function | Explanation |
| --- | --- |
| **contains** haystack needle<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `needle` occurs in `haystack`. |
| **startsWith** text prefix<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `text` starts with `prefix`. |
| **endsWith** text suffix<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `text` ends with `suffix`. |
| **indexOf** haystack needle<br><pre><code>`Text -> Text -> Option Int`</code></pre> | Returns the first index of `needle`, or `None` when not found. |
| **lastIndexOf** haystack needle<br><pre><code>`Text -> Text -> Option Int`</code></pre> | Returns the last index of `needle`, or `None` when not found. |
| **count** haystack needle<br><pre><code>`Text -> Text -> Int`</code></pre> | Returns the number of non-overlapping occurrences. |
| **compare** a b<br><pre><code>`Text -> Text -> Int`</code></pre> | Returns `-1`, `0`, or `1` in Unicode codepoint order (not locale-aware). |

Notes:
- `indexOf` and `lastIndexOf` return `None` when not found.

### Slicing and splitting

| Function | Explanation |
| --- | --- |
| **slice** start end text<br><pre><code>`Int -> Int -> Text -> Text`</code></pre> | Returns the substring from `start` (inclusive) to `end` (exclusive). |
| **split** sep text<br><pre><code>`Text -> Text -> List Text`</code></pre> | Splits `text` on `sep`. |
| **splitLines** text<br><pre><code>`Text -> List Text`</code></pre> | Splits on line endings. |
| **chunk** size text<br><pre><code>`Int -> Text -> List Text`</code></pre> | Splits into codepoint chunks of length `size`. |

Notes:
- `slice start end text` is half-open (`start` inclusive, `end` exclusive) and clamps out-of-range indices.
- `chunk` splits by codepoint count, not bytes.

### Trimming and padding

| Function | Explanation |
| --- | --- |
| **trim** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from both ends. |
| **trimStart** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from the start. |
| **trimEnd** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from the end. |
| **padStart** width fill text<br><pre><code>`Int -> Text -> Text -> Text`</code></pre> | Pads on the left to reach `width` using repeated `fill`. |
| **padEnd** width fill text<br><pre><code>`Int -> Text -> Text -> Text`</code></pre> | Pads on the right to reach `width` using repeated `fill`. |

Notes:
- `padStart width fill text` repeats `fill` as needed and truncates extra.

### Modification

| Function | Explanation |
| --- | --- |
| **replace** text needle replacement<br><pre><code>`Text -> Text -> Text -> Text`</code></pre> | Replaces the first occurrence of `needle`. |
| **replaceAll** text needle replacement<br><pre><code>`Text -> Text -> Text -> Text`</code></pre> | Replaces all occurrences of `needle`. |
| **remove** text needle<br><pre><code>`Text -> Text -> Text`</code></pre> | Removes all occurrences of `needle`. |
| **repeat** count text<br><pre><code>`Int -> Text -> Text`</code></pre> | Repeats `text` `count` times. |
| **reverse** text<br><pre><code>`Text -> Text`</code></pre> | Reverses grapheme clusters. |
| **concat** parts<br><pre><code>`List Text -> Text`</code></pre> | Concatenates all parts into one `Text`. |

Notes:
- `replace` changes the first occurrence only.
- `remove` is `replaceAll needle ""`.
- `reverse` is grapheme-aware and may be linear-time with extra allocations.

### Case and normalization

| Function | Explanation |
| --- | --- |
| **toLower** text<br><pre><code>`Text -> Text`</code></pre> | Converts to lowercase using Unicode rules. |
| **toUpper** text<br><pre><code>`Text -> Text`</code></pre> | Converts to uppercase using Unicode rules. |
| **capitalize** text<br><pre><code>`Text -> Text`</code></pre> | Uppercases the first grapheme and lowercases the rest. |
| **titleCase** text<br><pre><code>`Text -> Text`</code></pre> | Converts to title case using Unicode rules. |
| **caseFold** text<br><pre><code>`Text -> Text`</code></pre> | Produces a case-folded form for case-insensitive comparisons. |
| **normalizeNFC** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFC. |
| **normalizeNFD** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFD. |
| **normalizeNFKC** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFKC. |
| **normalizeNFKD** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFKD. |

### Encoding / decoding

| Function | Explanation |
| --- | --- |
| **toBytes** encoding text<br><pre><code>`Encoding -> Text -> Bytes`</code></pre> | Encodes `text` into `Bytes` using `encoding`. |
| **fromBytes** encoding bytes<br><pre><code>`Encoding -> Bytes -> Result TextError Text`</code></pre> | Decodes `bytes` and returns `TextError` on invalid input. |

### Formatting and conversion

| Function | Explanation |
| --- | --- |
| **toText** value<br><pre><code>`Show a => a -> Text`</code></pre> | Formats any `Show` instance into `Text`. |
| **parseInt** text<br><pre><code>`Text -> Option Int`</code></pre> | Parses a decimal integer, returning `None` on failure. |
| **parseFloat** text<br><pre><code>`Text -> Option Float`</code></pre> | Parses a decimal float, returning `None` on failure. |

## Usage Examples

```aivi
use aivi.text

slug = "  Hello World  "
clean = slug |> trim |> toLower |> replaceAll " " "-"

maybePort = parseInt "8080"
```


<!-- FILE: /05_stdlib/00_core/16_units -->

# Units Domain

The `Units` domain brings **Dimensional Analysis** to your code, solving the "Mars Climate Orbiter" problem. A bare number like `10` is dangerous—is it meters? seconds? kilograms? By attaching physical units to your values, AIVI understands the laws of physics at compile time. It knows that `Meters / Seconds = Speed`, but `Meters + Seconds` is nonsense, catching bugs before they ever run.

## Overview

```aivi
use aivi.units (Length, Time, Velocity)

// Define values with units attached
distance = 100.0m
time = 9.58s

// The compiler knows (Length / Time) results in Velocity
speed = distance / time 
// speed is now roughly 10.43 (m/s)
```

## Supported Dimensions

```aivi
Unit = { name: Text, factor: Float }
Quantity = { value: Float, unit: Unit }
```

## Domain Definition

```aivi
domain Units over Quantity = {
  (+) : Quantity -> Quantity -> Quantity
  (+) a b = { value: a.value + b.value, unit: a.unit }
  
  (-) : Quantity -> Quantity -> Quantity
  (-) a b = { value: a.value - b.value, unit: a.unit }
  
  (*) : Quantity -> Float -> Quantity
  (*) q s = { value: q.value * s, unit: q.unit }
  
  (/) : Quantity -> Float -> Quantity
  (/) q s = { value: q.value / s, unit: q.unit }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **defineUnit** name factor<br><pre><code>`Text -> Float -> Unit`</code></pre> | Creates a unit with a scale factor relative to the base unit. |
| **convert** quantity target<br><pre><code>`Quantity -> Unit -> Quantity`</code></pre> | Converts a quantity into the target unit. |
| **sameUnit** a b<br><pre><code>`Quantity -> Quantity -> Bool`</code></pre> | Returns whether two quantities share the same unit name. |

## Usage Examples

```aivi
use aivi.units

meter = defineUnit "m" 1.0
kilometer = defineUnit "km" 1000.0

distance = { value: 1500.0, unit: meter }
distanceKm = convert distance kilometer
```


<!-- FILE: /05_stdlib/00_core/24_regex -->

# Regex Domain

The `Regex` domain handles **Pattern Matching** for text. Whether you're validating emails, scraping data, or searching logs, simple substring checks often aren't enough. Regex gives you a powerful, concise language to describe *shapes* of text. AIVI's regex support is safe (checked at compile-time with `~r/.../`) and fast (compiling to native matching engines), so you don't have to worry about runtime crashes from bad patterns.

## Overview

```aivi
use aivi.regex (Regex)

email_pattern = ~r/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/
match = Regex.test(email_pattern, "user@example.com")

// With flags (example: case-insensitive)
email_ci = ~r/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/i
```

## Types

```aivi
type RegexError = InvalidPattern Text

Match = {
  full: Text,
  groups: List (Option Text),
  start: Int,
  end: Int
}
```

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **compile** pattern<br><pre><code>`Text -> Result RegexError Regex`</code></pre> | Builds a `Regex`, returning `RegexError` when invalid. |
| **test** regex text<br><pre><code>`Regex -> Text -> Bool`</code></pre> | Returns whether the regex matches anywhere in `text`. |
| **match** regex text<br><pre><code>`Regex -> Text -> Option Match`</code></pre> | Returns the first `Match` with capture groups. |
| **matches** regex text<br><pre><code>`Regex -> Text -> List Match`</code></pre> | Returns all matches in left-to-right order. |
| **find** regex text<br><pre><code>`Regex -> Text -> Option (Int, Int)`</code></pre> | Returns the first match byte index range. |
| **findAll** regex text<br><pre><code>`Regex -> Text -> List (Int, Int)`</code></pre> | Returns all match byte index ranges. |
| **split** regex text<br><pre><code>`Regex -> Text -> List Text`</code></pre> | Splits `text` on regex matches. |
| **replace** regex text replacement<br><pre><code>`Regex -> Text -> Text -> Text`</code></pre> | Replaces the first match. |
| **replaceAll** regex text replacement<br><pre><code>`Regex -> Text -> Text -> Text`</code></pre> | Replaces all matches. |

Notes:
- `match` returns the first match with capture groups (if any).
- `matches` returns all matches in left-to-right order.
- `replace` changes the first match only; `replaceAll` replaces all matches.
- Replacement strings support `$1`, `$2`, ... for capture groups.


<!-- FILE: /05_stdlib/00_core/27_testing -->

# Testing Domain

The `Testing` domain is built right into the language because reliability shouldn't be an afterthought. Instead of hunting for third-party runners or configuring complex suites, you can just write `@test` next to your code. It provides a standard, unified way to define, discover, and run tests, making sure your code does exactly what you think it does (and keeps doing it after you refactor).

## Overview

```aivi
use aivi.testing (assert, assertEq)

@test
additionWorks _ = {
    assertEq (1 + 1) 2
}
```

## Goals for v1.0

- `test` keyword or block construct.
- Assertions with rich diffs (`assertEq`, etc.).
- Test discovery and execution via `aivi test`.
- Property-based testing basics (generators) integration.


<!-- FILE: /05_stdlib/00_core/28_collections -->

# Collections Domain

The `Collections` domain is your toolbox for structured data. While `List` is great for simple sequences, real-world software needs more. Whether you need to look up users by their ID (Map), keep a list of unique tags (Set), or process tasks by priority (Heap), this domain provides persistent data structures designed for functional code.

## Overview

```aivi
use aivi.collections (Map, Set)

scores = Map.empty()
  |> Map.insert("Alice", 100)
  |> Map.insert("Bob", 95)

if scores |> Map.has("Alice") {
  print("Alice is present")
}
```

## v1.0 Scope

- **Map/Dict**: persistent ordered maps (AVL/Red-Black) and/or HashMaps (HAMT).
- **Set**: persistent sets corresponding to map types.
- **Queue/Deque**: efficient FIFO/LIFO structures.
- **Heap/PriorityQueue**.

## Literals and Merging (v1.0)

Collections introduce sigil-based literals for concise construction. These are domain literals and are validated at compile time.

### Map literal

```aivi
users = ~map{
  "id-1" => { name: "Alice", age: 30 }
  "id-2" => { name: "Bob", age: 25 }
}
```

Rules:
- Entries use `key => value`.
- Keys and values are full expressions.
- `...expr` spreads another map into the literal.
- When duplicate keys exist, the **last** entry wins (right-biased).

```aivi
defaults = ~map{ "theme" => "light", "lang" => "en" }
settings = ~map{ ...defaults, "theme" => "dark" }
```

### Set literal

```aivi
primes = ~set[2, 3, 5, 7, 11]
combined = ~set[...a, ...b]
```

Rules:
- Elements are expressions.
- `...expr` spreads another set.
- Duplicates are removed (set semantics).

### Merge operator

The `Collections` domain provides `++` as a right-biased merge for `Map` and union for `Set`.

```aivi
use aivi.collections (Map, Set, domain Collections)

merged = map1 ++ map2
allTags = set1 ++ set2
```

## Core API (v1.0)

The following functions are required for v1.0 implementations. Exact module layout may vary, but names and behavior should match.

### Map

| Function | Explanation |
| --- | --- |
| **Map.empty**<br><pre><code>`Map k v`</code></pre> | Creates an empty map. |
| **Map.size** map<br><pre><code>`Map k v -> Int`</code></pre> | Returns the number of entries. |
| **Map.has** key map<br><pre><code>`k -> Map k v -> Bool`</code></pre> | Returns whether `key` is present. |
| **Map.get** key map<br><pre><code>`k -> Map k v -> Option v`</code></pre> | Returns `Some value` or `None`. |
| **Map.insert** key value map<br><pre><code>`k -> v -> Map k v -> Map k v`</code></pre> | Returns a new map with the entry inserted. |
| **Map.update** key f map<br><pre><code>`k -> (v -> v) -> Map k v -> Map k v`</code></pre> | Applies `f` when `key` exists; otherwise no-op. |
| **Map.remove** key map<br><pre><code>`k -> Map k v -> Map k v`</code></pre> | Returns a new map without `key`. |
| **Map.map** f map<br><pre><code>`(v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms all values with `f`. |
| **Map.mapWithKey** f map<br><pre><code>`(k -> v -> v2) -> Map k v -> Map k v2`</code></pre> | Transforms values with access to keys. |
| **Map.keys** map<br><pre><code>`Map k v -> List k`</code></pre> | Returns all keys as a list. |
| **Map.values** map<br><pre><code>`Map k v -> List v`</code></pre> | Returns all values as a list. |
| **Map.entries** map<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Returns all entries as key/value pairs. |
| **Map.fromList** entries<br><pre><code>`List (k, v) -> Map k v`</code></pre> | Builds a map from key/value pairs. |
| **Map.toList** map<br><pre><code>`Map k v -> List (k, v)`</code></pre> | Converts a map into key/value pairs. |
| **Map.union** left right<br><pre><code>`Map k v -> Map k v -> Map k v`</code></pre> | Merges maps with right-biased keys. |

Notes:
- `Map.union` is right-biased (keys from the right map override).
- `Map.update` applies only when the key exists; otherwise it is a no-op.

### Set

| Function | Explanation |
| --- | --- |
| **Set.empty**<br><pre><code>`Set a`</code></pre> | Creates an empty set. |
| **Set.size** set<br><pre><code>`Set a -> Int`</code></pre> | Returns the number of elements. |
| **Set.has** value set<br><pre><code>`a -> Set a -> Bool`</code></pre> | Returns whether `value` is present. |
| **Set.insert** value set<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set with `value` inserted. |
| **Set.remove** value set<br><pre><code>`a -> Set a -> Set a`</code></pre> | Returns a new set without `value`. |
| **Set.union** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns the union of two sets. |
| **Set.intersection** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements common to both sets. |
| **Set.difference** left right<br><pre><code>`Set a -> Set a -> Set a`</code></pre> | Returns elements in `left` not in `right`. |
| **Set.fromList** values<br><pre><code>`List a -> Set a`</code></pre> | Builds a set from a list. |
| **Set.toList** set<br><pre><code>`Set a -> List a`</code></pre> | Converts a set into a list. |

### Queue / Deque

| Function | Explanation |
| --- | --- |
| **Queue.empty**<br><pre><code>`Queue a`</code></pre> | Creates an empty queue. |
| **Queue.enqueue** value queue<br><pre><code>`a -> Queue a -> Queue a`</code></pre> | Adds `value` to the back. |
| **Queue.dequeue** queue<br><pre><code>`Queue a -> Option (a, Queue a)`</code></pre> | Removes and returns the front value and remaining queue. |
| **Queue.peek** queue<br><pre><code>`Queue a -> Option a`</code></pre> | Returns the front value without removing it. |
| **Deque.empty**<br><pre><code>`Deque a`</code></pre> | Creates an empty deque. |
| **Deque.pushFront** value deque<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the front. |
| **Deque.pushBack** value deque<br><pre><code>`a -> Deque a -> Deque a`</code></pre> | Adds `value` to the back. |
| **Deque.popFront** deque<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the front value and rest. |
| **Deque.popBack** deque<br><pre><code>`Deque a -> Option (a, Deque a)`</code></pre> | Removes and returns the back value and rest. |
| **Deque.peekFront** deque<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the front value without removing it. |
| **Deque.peekBack** deque<br><pre><code>`Deque a -> Option a`</code></pre> | Returns the back value without removing it. |

### Heap / PriorityQueue

| Function | Explanation |
| --- | --- |
| **Heap.empty**<br><pre><code>`Heap a`</code></pre> | Creates an empty heap. |
| **Heap.push** value heap<br><pre><code>`a -> Heap a -> Heap a`</code></pre> | Inserts `value` into the heap. |
| **Heap.popMin** heap<br><pre><code>`Heap a -> Option (a, Heap a)`</code></pre> | Removes and returns the smallest value and remaining heap. |
| **Heap.peekMin** heap<br><pre><code>`Heap a -> Option a`</code></pre> | Returns the smallest value without removing it. |

`Heap` ordering is determined by `Ord` for the element type.


<!-- FILE: /05_stdlib/01_math/01_math -->

# Math Module

The `aivi.math` module provides standard numeric functions and constants for `Int` and `Float`.
It is intentionally small, predictable, and aligned with common math libraries across languages.

## Overview

```aivi
use aivi.math

area = pi * r * r
clamped = clamp 0.0 1.0 x
```

## Constants

```aivi
pi : Float
tau : Float
e : Float
inf : Float
nan : Float
phi : Float
sqrt2 : Float
ln2 : Float
ln10 : Float
```

## Angles

Angles are represented by a dedicated domain so trigonometric functions are not called with raw `Float` values.

```aivi
Angle = { radians: Float }
```

| Function | Explanation |
| --- | --- |
| **radians** value<br><pre><code>`Float -> Angle`</code></pre> | Creates an `Angle` from a raw radians value. |

| Function | Explanation |
| --- | --- |
| **degrees** value<br><pre><code>`Float -> Angle`</code></pre> | Creates an `Angle` from a raw degrees value. |

| Function | Explanation |
| --- | --- |
| **toRadians** angle<br><pre><code>`Angle -> Float`</code></pre> | Extracts the radians value from an `Angle`. |

| Function | Explanation |
| --- | --- |
| **toDegrees** angle<br><pre><code>`Angle -> Float`</code></pre> | Extracts the degrees value from an `Angle`. |

## Basic helpers

| Function | Explanation |
| --- | --- |
| **abs** value<br><pre><code>`Int -> Int`</code></pre> | Returns the absolute value of `value`. |
| **abs** value<br><pre><code>`Float -> Float`</code></pre> | Returns the absolute value of `value`. |

| Function | Explanation |
| --- | --- |
| **sign** x<br><pre><code>`Float -> Float`</code></pre> | Returns `-1.0`, `0.0`, or `1.0` based on the sign of `x`. |
| **copysign** mag sign<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns `mag` with the sign of `sign`. |

| Function | Explanation |
| --- | --- |
| **min** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the smaller of `a` and `b`. |
| **max** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the larger of `a` and `b`. |
| **minAll** values<br><pre><code>`List Float -> Option Float`</code></pre> | Returns the minimum of `values` or `None` when empty. |
| **maxAll** values<br><pre><code>`List Float -> Option Float`</code></pre> | Returns the maximum of `values` or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **clamp** low high x<br><pre><code>`Float -> Float -> Float -> Float`</code></pre> | Limits `x` to the closed interval `[low, high]`. |
| **sum** values<br><pre><code>`List Float -> Float`</code></pre> | Sums values (empty list yields `0.0`). |
| **sumInt** values<br><pre><code>`List Int -> Int`</code></pre> | Sums values (empty list yields `0`). |

## Rounding and decomposition

| Function | Explanation |
| --- | --- |
| **floor** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `-inf`. |
| **ceil** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `+inf`. |
| **trunc** x<br><pre><code>`Float -> Float`</code></pre> | Rounds toward `0`. |
| **round** x<br><pre><code>`Float -> Float`</code></pre> | Uses banker's rounding (ties to even). |
| **fract** x<br><pre><code>`Float -> Float`</code></pre> | Returns the fractional part with the same sign as `x`. |

| Function | Explanation |
| --- | --- |
| **modf** x<br><pre><code>`Float -> (Float, Float)`</code></pre> | Returns `(intPart, fracPart)` where `x = intPart + fracPart`. |
| **frexp** x<br><pre><code>`Float -> (Float, Int)`</code></pre> | Returns `(mantissa, exponent)` such that `x = mantissa * 2^exponent`. |
| **ldexp** mantissa exponent<br><pre><code>`Float -> Int -> Float`</code></pre> | Computes `mantissa * 2^exponent`. |

## Powers, roots, and logs

| Function | Explanation |
| --- | --- |
| **pow** base exp<br><pre><code>`Float -> Float -> Float`</code></pre> | Raises `base` to `exp`. |
| **sqrt** x<br><pre><code>`Float -> Float`</code></pre> | Computes the square root. |
| **cbrt** x<br><pre><code>`Float -> Float`</code></pre> | Computes the cube root. |
| **hypot** x y<br><pre><code>`Float -> Float -> Float`</code></pre> | Computes `sqrt(x*x + y*y)` with reduced overflow/underflow. |

| Function | Explanation |
| --- | --- |
| **exp** x<br><pre><code>`Float -> Float`</code></pre> | Computes `e^x`. |
| **exp2** x<br><pre><code>`Float -> Float`</code></pre> | Computes `2^x`. |
| **expm1** x<br><pre><code>`Float -> Float`</code></pre> | Computes `e^x - 1` with improved precision near zero. |

| Function | Explanation |
| --- | --- |
| **log** x<br><pre><code>`Float -> Float`</code></pre> | Computes the natural log. |
| **log10** x<br><pre><code>`Float -> Float`</code></pre> | Computes the base-10 log. |
| **log2** x<br><pre><code>`Float -> Float`</code></pre> | Computes the base-2 log. |
| **log1p** x<br><pre><code>`Float -> Float`</code></pre> | Computes `log(1 + x)` with improved precision near zero. |

## Trigonometry

| Function | Explanation |
| --- | --- |
| **sin** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the sine ratio for `angle`. |
| **cos** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the cosine ratio for `angle`. |
| **tan** angle<br><pre><code>`Angle -> Float`</code></pre> | Computes the tangent ratio for `angle`. |

| Function | Explanation |
| --- | --- |
| **asin** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose sine is `x`. |
| **acos** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose cosine is `x`. |
| **atan** x<br><pre><code>`Float -> Angle`</code></pre> | Returns the angle whose tangent is `x`. |
| **atan2** y x<br><pre><code>`Float -> Float -> Angle`</code></pre> | Returns the angle of the vector `(x, y)` from the positive x-axis. |

## Hyperbolic functions

| Function | Explanation |
| --- | --- |
| **sinh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic sine. |
| **cosh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic cosine. |
| **tanh** x<br><pre><code>`Float -> Float`</code></pre> | Computes hyperbolic tangent. |
| **asinh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic sine. |
| **acosh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic cosine. |
| **atanh** x<br><pre><code>`Float -> Float`</code></pre> | Computes inverse hyperbolic tangent. |

## Integer math

| Function | Explanation |
| --- | --- |
| **gcd** a b<br><pre><code>`Int -> Int -> Int`</code></pre> | Computes the greatest common divisor. |
| **lcm** a b<br><pre><code>`Int -> Int -> Int`</code></pre> | Computes the least common multiple. |
| **gcdAll** values<br><pre><code>`List Int -> Option Int`</code></pre> | Returns the gcd of all values or `None` when empty. |
| **lcmAll** values<br><pre><code>`List Int -> Option Int`</code></pre> | Returns the lcm of all values or `None` when empty. |

| Function | Explanation |
| --- | --- |
| **factorial** n<br><pre><code>`Int -> BigInt`</code></pre> | Computes `n!`. |
| **comb** n k<br><pre><code>`Int -> Int -> BigInt`</code></pre> | Computes combinations ("n choose k"). |
| **perm** n k<br><pre><code>`Int -> Int -> BigInt`</code></pre> | Computes permutations ("n P k"). |

| Function | Explanation |
| --- | --- |
| **divmod** | `Int -> Int -> (Int, Int)` | `a`: dividend; `b`: divisor. | Returns `(q, r)` where `a = q * b + r` and `0 <= r < |b|`. |
| **modPow** | `Int -> Int -> Int -> Int` | `base`: base; `exp`: exponent; `modulus`: modulus. | Computes `(base^exp) mod modulus`. |

Notes:
- `BigInt` is from `aivi.number.bigint` and is re-exported by `aivi.math`.

## Floating-point checks

| Function | Explanation |
| --- | --- |
| **isFinite** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is finite. |
| **isInf** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is infinite. |
| **isNaN** x<br><pre><code>`Float -> Bool`</code></pre> | Returns whether `x` is NaN. |
| **nextAfter** from to<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the next representable float after `from` toward `to`. |
| **ulp** x<br><pre><code>`Float -> Float`</code></pre> | Returns the size of one unit-in-the-last-place at `x`. |

## Remainders

| Function | Explanation |
| --- | --- |
| **fmod** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the remainder using truncation toward zero. |
| **remainder** a b<br><pre><code>`Float -> Float -> Float`</code></pre> | Returns the IEEE-754 remainder (round-to-nearest quotient). |

## Usage Examples

```aivi
use aivi.math

angle = degrees 90.0
unit = sin angle

digits = [1.0, 2.0, 3.0] |> sum
```


<!-- FILE: /05_stdlib/01_math/05_vector -->

# Vector Domain

The `Vector` domain handles 2D and 3D vectors (`Vec2`, `Vec3`), the fundamental atoms of spatial math.

A **Vector** is just a number with a direction. It's the difference between saying "10 miles" (Scalar) and "10 miles North" (Vector).
*   **Position**: "Where am I?" (Point)
*   **Velocity**: "Where am I going?" (Movement)
*   **Force**: "What's pushing me?" (Physics)

Graphics and physics use vectors for clean math (`v1 + v2`) and benefit from hardware acceleration (SIMD).

## Overview

```aivi
use aivi.vector (Vec2, Vec3)

// Define using the `v2` tag
v1 = (1.0, 2.0)v2
v2 = (3.0, 4.0)v2

// Add components parallelly
v3 = v1 + v2 // (4.0, 6.0)
```


## Features

```aivi
Vec2 = { x: Float, y: Float }
Vec3 = { x: Float, y: Float, z: Float }
Vec4 = { x: Float, y: Float, z: Float, w: Float }

Scalar = Float
```

## Domain Definition

```aivi
domain Vector over Vec2 = {
  (+) : Vec2 -> Vec2 -> Vec2
  (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y }
  
  (-) : Vec2 -> Vec2 -> Vec2
  (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y }
  
  (*) : Vec2 -> Scalar -> Vec2
  (*) v s = { x: v.x * s, y: v.y * s }
  
  (/) : Vec2 -> Scalar -> Vec2
  (/) v s = { x: v.x / s, y: v.y / s }
}

domain Vector over Vec3 = {
  (+) : Vec3 -> Vec3 -> Vec3
  (+) v1 v2 = { x: v1.x + v2.x, y: v1.y + v2.y, z: v1.z + v2.z }
  
  (-) : Vec3 -> Vec3 -> Vec3
  (-) v1 v2 = { x: v1.x - v2.x, y: v1.y - v2.y, z: v1.z - v2.z }
  
  (*) : Vec3 -> Scalar -> Vec3
  (*) v s = { x: v.x * s, y: v.y * s, z: v.z * s }
  
  (/) : Vec3 -> Scalar -> Vec3
  (/) v s = { x: v.x / s, y: v.y / s, z: v.z / s }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **magnitude** v<br><pre><code>`Vec2 -> Float`</code></pre> | Returns the Euclidean length of `v`. |
| **normalize** v<br><pre><code>`Vec2 -> Vec2`</code></pre> | Returns a unit vector in the direction of `v`. |
| **dot** a b<br><pre><code>`Vec2 -> Vec2 -> Float`</code></pre> | Returns the dot product of `a` and `b`. |
| **cross** a b<br><pre><code>`Vec3 -> Vec3 -> Vec3`</code></pre> | Returns the 3D cross product orthogonal to `a` and `b`. |

## Usage Examples

```aivi
use aivi.vector

position = { x: 10.0, y: 20.0 }
velocity = { x: 1.0, y: 0.5 }

newPos = position + velocity * 0.016  // 60fps frame
direction = normalize velocity
```


<!-- FILE: /05_stdlib/01_math/09_matrix -->

# Matrix Domain

The `Matrix` domain provides grids of numbers (`Mat3`, `Mat4`) used primarily for **Transformations**.

Think of a Matrix as a "teleporter instruction set" for points. A single 4x4 grid can bundle up a complex recipe of movements: "Rotate 30 degrees, scale up by 200%, and move 5 units left."

Manually calculating the new position of a 3D point after it's been rotated, moved, and scaled is incredibly complex algebra. Matrices simplify this to `Point * Matrix`. They are the mathematical engine behind every 3D game and renderer.

## Overview

```aivi
use aivi.matrix (Mat4)

// A generic "identity" matrix (does nothing)
m = Mat4.identity()

// Create a instruction to move 10 units X
translation = Mat4.translate(10.0, 0.0, 0.0)

// Combine them
m_prime = m * translation
```


## Features

```aivi
Mat2 = { m00: Float, m01: Float, m10: Float, m11: Float }
Mat3 = {
  m00: Float, m01: Float, m02: Float,
  m10: Float, m11: Float, m12: Float,
  m20: Float, m21: Float, m22: Float
}
Mat4 = {
  m00: Float, m01: Float, m02: Float, m03: Float,
  m10: Float, m11: Float, m12: Float, m13: Float,
  m20: Float, m21: Float, m22: Float, m23: Float,
  m30: Float, m31: Float, m32: Float, m33: Float
}

Scalar = Float
```

## Domain Definition

```aivi
domain Matrix over Mat2 = {
  (+) : Mat2 -> Mat2 -> Mat2
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11
  }
  
  (-) : Mat2 -> Mat2 -> Mat2
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11
  }
  
  (*) : Mat2 -> Scalar -> Mat2
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s,
    m10: m.m10 * s, m11: m.m11 * s
  }
}

domain Matrix over Mat3 = {
  (+) : Mat3 -> Mat3 -> Mat3
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22
  }
  
  (-) : Mat3 -> Mat3 -> Mat3
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22
  }
  
  (*) : Mat3 -> Scalar -> Mat3
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s
  }
}

domain Matrix over Mat4 = {
  (+) : Mat4 -> Mat4 -> Mat4
  (+) a b = {
    m00: a.m00 + b.m00, m01: a.m01 + b.m01, m02: a.m02 + b.m02, m03: a.m03 + b.m03,
    m10: a.m10 + b.m10, m11: a.m11 + b.m11, m12: a.m12 + b.m12, m13: a.m13 + b.m13,
    m20: a.m20 + b.m20, m21: a.m21 + b.m21, m22: a.m22 + b.m22, m23: a.m23 + b.m23,
    m30: a.m30 + b.m30, m31: a.m31 + b.m31, m32: a.m32 + b.m32, m33: a.m33 + b.m33
  }
  
  (-) : Mat4 -> Mat4 -> Mat4
  (-) a b = {
    m00: a.m00 - b.m00, m01: a.m01 - b.m01, m02: a.m02 - b.m02, m03: a.m03 - b.m03,
    m10: a.m10 - b.m10, m11: a.m11 - b.m11, m12: a.m12 - b.m12, m13: a.m13 - b.m13,
    m20: a.m20 - b.m20, m21: a.m21 - b.m21, m22: a.m22 - b.m22, m23: a.m23 - b.m23,
    m30: a.m30 - b.m30, m31: a.m31 - b.m31, m32: a.m32 - b.m32, m33: a.m33 - b.m33
  }
  
  (*) : Mat4 -> Scalar -> Mat4
  (*) m s = {
    m00: m.m00 * s, m01: m.m01 * s, m02: m.m02 * s, m03: m.m03 * s,
    m10: m.m10 * s, m11: m.m11 * s, m12: m.m12 * s, m13: m.m13 * s,
    m20: m.m20 * s, m21: m.m21 * s, m22: m.m22 * s, m23: m.m23 * s,
    m30: m.m30 * s, m31: m.m31 * s, m32: m.m32 * s, m33: m.m33 * s
  }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **identity2**<br><pre><code>`Mat2`</code></pre> | Identity matrix for 2x2. |
| **identity3**<br><pre><code>`Mat3`</code></pre> | Identity matrix for 3x3. |
| **identity4**<br><pre><code>`Mat4`</code></pre> | Identity matrix for 4x4. |
| **transpose2** m<br><pre><code>`Mat2 -> Mat2`</code></pre> | Flips rows and columns of a 2x2. |
| **transpose3** m<br><pre><code>`Mat3 -> Mat3`</code></pre> | Flips rows and columns of a 3x3. |
| **transpose4** m<br><pre><code>`Mat4 -> Mat4`</code></pre> | Flips rows and columns of a 4x4. |
| **multiply2** a b<br><pre><code>`Mat2 -> Mat2 -> Mat2`</code></pre> | Multiplies two 2x2 matrices. |
| **multiply3** a b<br><pre><code>`Mat3 -> Mat3 -> Mat3`</code></pre> | Multiplies two 3x3 matrices. |
| **multiply4** a b<br><pre><code>`Mat4 -> Mat4 -> Mat4`</code></pre> | Multiplies two 4x4 matrices. |

## Usage Examples

```aivi
use aivi.matrix

scale2 = { m00: 2.0, m01: 0.0, m10: 0.0, m11: 2.0 }
rotate2 = { m00: 0.0, m01: -1.0, m10: 1.0, m11: 0.0 }

combined = multiply2 scale2 rotate2
unit = combined * 0.5
```


<!-- FILE: /05_stdlib/01_math/10_number -->

# Number Domains (BigInt, Rational, Complex)

The `aivi.number` family groups numeric domains that sit above `Int` and `Float`:

- `aivi.number.bigint` for arbitrary-precision integers
- `aivi.number.rational` for exact fractions
- `aivi.number.complex` for complex arithmetic

You can use either the facade module or the specific domain module depending on how much you want in scope.

```aivi
// Facade (types + helpers)
use aivi.number

// Domain modules (operators + literals)
use aivi.number.bigint
use aivi.number.rational
use aivi.number.complex
```


## BigInt

`BigInt` is an **opaque native type** for arbitrary-precision integers.

```aivi
// Native type (backed by Rust BigInt or similar)
type BigInt
```

```aivi
domain BigInt over BigInt = {
  (+) : BigInt -> BigInt -> BigInt
  (-) : BigInt -> BigInt -> BigInt
  (*) : BigInt -> BigInt -> BigInt

  1n = fromInt 1
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **fromInt** value<br><pre><code>`Int -> BigInt`</code></pre> | Converts a machine `Int` into `BigInt`. |
| **toInt** value<br><pre><code>`BigInt -> Int`</code></pre> | Converts a `BigInt` to `Int` (may overflow in implementations). |

Example:

```aivi
use aivi.number.bigint

huge = 10_000_000_000_000_000_000_000n
sum = huge + 1n
```

## Decimal

`Decimal` is an **opaque native type** for fixed-point arithmetic (base-10), suitable for financial calculations where `Float` precision errors are unacceptable.

```aivi
// Native type (backed by Rust Decimal or similar)
type Decimal
```

```aivi
domain Decimal over Decimal = {
  (+) : Decimal -> Decimal -> Decimal
  (-) : Decimal -> Decimal -> Decimal
  (*) : Decimal -> Decimal -> Decimal
  (/) : Decimal -> Decimal -> Decimal

  // Literal suffix 'dec'
  1.0dec = fromFloat 1.0
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **fromFloat** value<br><pre><code>`Float -> Decimal`</code></pre> | Converts a `Float` into `Decimal` using base-10 rounding rules. |
| **toFloat** value<br><pre><code>`Decimal -> Float`</code></pre> | Converts a `Decimal` into a `Float`. |
| **round** value places<br><pre><code>`Decimal -> Int -> Decimal`</code></pre> | Rounds to `places` decimal digits. |

Example:

```aivi
use aivi.number.decimal

price = 19.99dec
tax = price * 0.2dec
total = price + tax
```

## Rational

`Rational` is an **opaque native type** for exact fractions (`num/den`).

```aivi
// Native type (backed by Rust Rational or similar)
type Rational
```

```aivi
domain Rational over Rational = {
  (+) : Rational -> Rational -> Rational
  (-) : Rational -> Rational -> Rational
  (*) : Rational -> Rational -> Rational
  (/) : Rational -> Rational -> Rational
}
```

Helpers:

| Function | Explanation |
| --- | --- |
| **normalize** r<br><pre><code>`Rational -> Rational`</code></pre> | Reduces a fraction to lowest terms. |
| **numerator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the numerator. |
| **denominator** r<br><pre><code>`Rational -> BigInt`</code></pre> | Returns the denominator. |

Example:

```aivi
use aivi.number.rational

// exact 1/2
half = normalize (fromInt 1 / fromInt 2) 
sum = half + half
```

## Complex

`Complex` represents values of the form `a + bi`. It is typically a struct of two floats, but domain operations are backed by optimized native implementations.

```aivi
Complex = { re: Float, im: Float }
i : Complex
```

```aivi
domain Complex over Complex = {
  (+) : Complex -> Complex -> Complex
  (+) a b = { re: a.re + b.re, im: a.im + b.im }

  (-) : Complex -> Complex -> Complex
  (-) a b = { re: a.re - b.re, im: a.im - b.im }

  (*) : Complex -> Complex -> Complex
  (*) a b = {
    re: a.re * b.re - a.im * b.im,
    im: a.re * b.im + a.im * b.re
  }

  (/) : Complex -> Float -> Complex
  (/) z s = { re: z.re / s, im: z.im / s }
}
```

Example:

```aivi
use aivi.number.complex

z1 = 3.0 + 4.0 * i
z2 = 1.0 - 2.0 * i
sum = z1 + z2
```

## Quaternion

The `Quaternion` domain provides tools for handling **3D rotations** without gimbal lock.

```aivi
use aivi.number.quaternion (Quat)

// Rotate 90 degrees around the Y (up) axis
q1 = Quat.fromEuler(0.0, 90.0, 0.0)

// The "identity" quaternion means "no rotation"
q2 = Quat.identity()

// Smoothly transition halfway between "no rotation" and "90 degrees"
interpolated = Quat.slerp(q1, q2, 0.5)
```

```aivi
Quaternion = { w: Float, x: Float, y: Float, z: Float }
```

```aivi
domain Quaternion over Quaternion = {
  (+) : Quaternion -> Quaternion -> Quaternion
  (+) a b = { w: a.w + b.w, x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }

  (-) : Quaternion -> Quaternion -> Quaternion
  (-) a b = { w: a.w - b.w, x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }

  (*) : Quaternion -> Quaternion -> Quaternion
  (*) a b = {
    w: a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    x: a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
    y: a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
    z: a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w
  }

  (/) : Quaternion -> Float -> Quaternion
  (/) q s = { w: q.w / s, x: q.x / s, y: q.y / s, z: q.z / s }
}
```

| Function | Explanation |
| --- | --- |
| **fromAxisAngle** axis theta<br><pre><code>`{ x: Float, y: Float, z: Float } -> Float -> Quaternion`</code></pre> | Creates a rotation from axis/angle. |
| **conjugate** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Negates the vector part. |
| **magnitude** q<br><pre><code>`Quaternion -> Float`</code></pre> | Returns the quaternion length. |
| **normalize** q<br><pre><code>`Quaternion -> Quaternion`</code></pre> | Returns a unit-length quaternion. |

```aivi
use aivi.number.quaternion

axis = { x: 0.0, y: 1.0, z: 0.0 }
spin = fromAxisAngle axis 1.570796

unit = normalize spin
```


<!-- FILE: /05_stdlib/01_math/13_probability -->

# Probability & Distribution Domain

The `Probability` domain gives you tools for **Statistical Distributions** and structured randomness.

Standard `random()` just gives you a boring uniform number between 0 and 1. But reality isn't uniform.
*   Heights of people follow a **Bell Curve** (Normal distribution).
*   Radioactive decay follows a **Poisson** distribution.
*   Success/failure rates follow a **Bernoulli** distribution.

This domain lets you define the *shape* of the chaotic world you want to simulate, and then draw mathematically correct samples from it.

## Overview

```aivi
use aivi.probability (Normal, uniform)

// Create a Bell curve centered at 0 with standard deviation of 1
distribution = Normal(0.0, 1.0) 

// Get a random number that fits this curve
// (Most values will be near 0, few will be near -3 or 3)
sample = distribution |> sample()
```


## Features

```aivi
Probability = Float
Distribution a = { pdf: a -> Probability }
```

## Domain Definition

```aivi
domain Probability over Probability = {
  (+) : Probability -> Probability -> Probability
  (-) : Probability -> Probability -> Probability
  (*) : Probability -> Probability -> Probability
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **clamp** p<br><pre><code>`Probability -> Probability`</code></pre> | Bounds `p` into `[0.0, 1.0]`. |
| **bernoulli** p<br><pre><code>`Probability -> Distribution Bool`</code></pre> | Creates a distribution over `Bool` with success probability `p`. |
| **uniform** lo hi<br><pre><code>`Float -> Float -> Distribution Float`</code></pre> | Creates a uniform distribution over `[lo, hi]`. |
| **expectation** dist x<br><pre><code>`Distribution Float -> Float -> Float`</code></pre> | Returns the contribution of `x` to the expected value. |

## Usage Examples

```aivi
use aivi.probability

p = clamp 0.7
coin = bernoulli p
probHeads = coin.pdf true
```


<!-- FILE: /05_stdlib/01_math/14_signal -->

# FFT & Signal Domain

The `Signal` domain provides tools for **Digital Signal Processing** (DSP), including the Fast Fourier Transform.

Signals are everything: audio from a mic, vibrations in a bridge, or stock market prices.
*   **Time Domain**: "How loud is it right now?"
*   **Frequency Domain**: "What notes are being played?"

The **Fast Fourier Transform (FFT)** is a legendary algorithm that converts Time into Frequency. It lets you unbake a cake to find the ingredients. If you want to filter noise from audio, analyze heartbeats, or compress images, you need this domain.

## Overview

```aivi
use aivi.signal (fft, ifft)

// A simple signal (e.g., audio samples)
timeDomain = [1.0, 0.5, 0.25, 0.125]

// Convert to frequencies to analyze pitch
freqDomain = fft(timeDomain)
```


## Features

```aivi
Signal = { samples: List Float, rate: Float }
Spectrum = { bins: List Complex, rate: Float }
```

## Domain Definition

```aivi
domain Signal over Signal = {
  (+) : Signal -> Signal -> Signal
  (+) a b = { samples: zipWith (+) a.samples b.samples, rate: a.rate }
  
  (*) : Signal -> Float -> Signal
  (*) s k = { samples: map (\x -> x * k) s.samples, rate: s.rate }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **fft** signal<br><pre><code>`Signal -> Spectrum`</code></pre> | Transforms a signal into a frequency-domain spectrum. |
| **ifft** spectrum<br><pre><code>`Spectrum -> Signal`</code></pre> | Reconstructs a time-domain signal from its spectrum. |
| **windowHann** signal<br><pre><code>`Signal -> Signal`</code></pre> | Applies a Hann window to reduce spectral leakage. |
| **normalize** signal<br><pre><code>`Signal -> Signal`</code></pre> | Scales samples so the max absolute value is `1.0`. |

## Usage Examples

```aivi
use aivi.signal
use aivi.number.complex

audio = { samples: [0.0, 0.5, 1.0, 0.5], rate: 44100.0 }
spectrum = fft audio
recon = ifft spectrum
```


<!-- FILE: /05_stdlib/01_math/15_geometry -->

# Geometry Domain

The `Geometry` domain creates shapes (`Sphere`, `Ray`, `Rect`) and checks if they touch.

This is the "physical" side of math. While `Vector` handles movement, `Geometry` handles **stuff**.
*   "Did I click the button?" (Point vs Rect)
*   "Did the bullet hit the player?" (Ray vs Cylinder)
*   "Is the tank inside the base?" (Point vs Polygon)

Almost every visual application needs to know when two things collide. This domain gives you standard shapes and highly optimized algorithms to check for intersections instantly.

## Overview

```aivi
use aivi.geometry (Ray, Sphere, intersect)

// A ray firing forwards from origin
ray = Ray(origin: {x:0, y:0, z:0}, dir: {x:0, y:0, z:1})

// A sphere 5 units away
sphere = Sphere(center: {x:0, y:0, z:5}, radius: 1.0)

if intersect(ray, sphere) {
    print("Hit!")
}
```


## Features

```aivi
Point2 = { x: Float, y: Float }
Point3 = { x: Float, y: Float, z: Float }
Line2 = { origin: Point2, direction: Point2 }
Segment2 = { start: Point2, end: Point2 }
Polygon = { vertices: List Point2 }
```

## Domain Definition

```aivi
domain Geometry over Point2 = {
  (+) : Point2 -> Point2 -> Point2
  (+) a b = { x: a.x + b.x, y: a.y + b.y }
  
  (-) : Point2 -> Point2 -> Point2
  (-) a b = { x: a.x - b.x, y: a.y - b.y }
}

domain Geometry over Point3 = {
  (+) : Point3 -> Point3 -> Point3
  (+) a b = { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
  
  (-) : Point3 -> Point3 -> Point3
  (-) a b = { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **distance** a b<br><pre><code>`Point2 -> Point2 -> Float`</code></pre> | Returns the Euclidean distance between two 2D points. |
| **midpoint** segment<br><pre><code>`Segment2 -> Point2`</code></pre> | Returns the center point of a line segment. |
| **area** polygon<br><pre><code>`Polygon -> Float`</code></pre> | Returns the signed area (positive for counter-clockwise winding). |

## Usage Examples

```aivi
use aivi.geometry

p1 = { x: 0.0, y: 0.0 }
p2 = { x: 3.0, y: 4.0 }

d = distance p1 p2
center = midpoint { start: p1, end: p2 }
```


<!-- FILE: /05_stdlib/01_math/17_graph -->

# Graph Domain

The `Graph` domain is for modelling **Relationships** and **Networks**.

In computer science, a "Graph" isn't a pie chart. It's a map of connections:
*   **Social Networks**: People connected by Friendships.
*   **Maps**: Cities connected by Roads.
*   **The Internet**: Pages connected by Links.

If you need to find the shortest path between two points or see who is friends with whom, you need a Graph. This domain provides the data structures and algorithms (like BFS and Dijkstra) to solve these problems efficiently.

## Overview

```aivi
use aivi.graph (Graph, bfs)

// Create a small network
g = Graph.fromEdges([
  (1, 2),  // Node 1 connects to 2
  (2, 3),  // Node 2 connects to 3
  (1, 3)   // Node 1 connects to 3
])

// Find a path through the network
path = bfs(g, start: 1, end: 3)
```


## Features

```aivi
NodeId = Int
Edge = { from: NodeId, to: NodeId, weight: Float }
Graph = { nodes: List NodeId, edges: List Edge }
```

## Domain Definition

```aivi
domain Graph over Graph = {
  (+) : Graph -> Graph -> Graph
  (+) a b = { nodes: unique (a.nodes ++ b.nodes), edges: a.edges ++ b.edges }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **addEdge** graph edge<br><pre><code>`Graph -> Edge -> Graph`</code></pre> | Returns a new graph with the edge added and nodes updated. |
| **neighbors** graph node<br><pre><code>`Graph -> NodeId -> List NodeId`</code></pre> | Returns the outgoing neighbors of `node`. |
| **shortestPath** graph start goal<br><pre><code>`Graph -> NodeId -> NodeId -> List NodeId`</code></pre> | Returns the node path computed by Dijkstra. |

## Usage Examples

```aivi
use aivi.graph

g0 = { nodes: [], edges: [] }
g1 = addEdge g0 { from: 1, to: 2, weight: 1.0 }
g2 = addEdge g1 { from: 2, to: 3, weight: 2.0 }

path = shortestPath g2 1 3
```


<!-- FILE: /05_stdlib/01_math/18_linear_algebra -->

# Linear Algebra Domain

The `LinearAlgebra` domain solves massive **Systems of Equations**.

While `Vector` and `Matrix` are for 3D graphics, this domain is for "hard" science and engineering. It answers questions like: "If `3x + 2y = 10` and `x - y = 5`, what are `x` and `y`?"... but for systems with *thousands* of variables.

Whether you're simulating heat flow across a computer chip, calculating structural loads on a bridge, or training a neural network, you are solving systems of linear equations. This domain wraps industrial-grade solvers (like LAPACK) to do the heavy lifting for you.

## Overview

```aivi
use aivi.linalg (solve, eigen)

// Matrix A and Vector b
// Solve for x in: Ax = b
// (Finds the inputs that produce the known output)
x = solve(A, b)
```


## Features

```aivi
Vec = { size: Int, data: List Float }
Mat = { rows: Int, cols: Int, data: List Float }
```

## Domain Definition

```aivi
domain LinearAlgebra over Vec = {
  (+) : Vec -> Vec -> Vec
  (+) a b = { size: a.size, data: zipWith (+) a.data b.data }
  
  (-) : Vec -> Vec -> Vec
  (-) a b = { size: a.size, data: zipWith (-) a.data b.data }
  
  (*) : Vec -> Float -> Vec
  (*) v s = { size: v.size, data: map (\x -> x * s) v.data }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **dot** a b<br><pre><code>`Vec -> Vec -> Float`</code></pre> | Returns the dot product of two vectors. |
| **matMul** a b<br><pre><code>`Mat -> Mat -> Mat`</code></pre> | Multiplies matrices (rows of `a` by columns of `b`). |
| **solve2x2** m v<br><pre><code>`Mat -> Vec -> Vec`</code></pre> | Solves the system `m * x = v`. |

## Usage Examples

```aivi
use aivi.linear_algebra

v1 = { size: 3, data: [1.0, 2.0, 3.0] }
v2 = { size: 3, data: [4.0, 5.0, 6.0] }

prod = dot v1 v2
```


<!-- FILE: /05_stdlib/02_chronos/02_calendar -->

# Calendar Domain

The `Calendar` domain gives you robust tools for handling **Dates** and **Human Time**.

Handling time is deceptively hard. Ideally, a day is 24 hours. In reality, months have 28-31 days, years have 365 or 366 days, and timezones shift clocks back and forth.

The `Calendar` domain hides this chaos. Writing `timestamp + 86400` works until a leap second deletes your data. This domain ensures that when you say "Next Month," it handles the math correctly—whether it's February or July—making your scheduling logic reliable and legible.

## Overview

```aivi
use aivi.calendar (Date, DateTime)

now = DateTime.now()

birthday = ~d(1990-12-31)
timestamp = ~dt(2025-02-08T12:34:56Z)

// "Human" math: Add 7 days, regardless of seconds
next_week = now + 7days
```

## Features

```aivi
Date = { year: Int, month: Int, day: Int }

EndOfMonth = EndOfMonth
```

## Domain Definition

```aivi
domain Calendar over Date = {
  type Delta = Day Int | Month Int | Year Int | End EndOfMonth
  
  // Add delta to date
  (+) : Date -> Delta -> Date
  (+) date (Day n)   = addDays date n
  (+) date (Month n) = addMonths date n
  (+) date (Year n)  = addYears date n
  (+) date End       = endOfMonth date
  
  // Subtract delta from date
  (-) : Date -> Delta -> Date
  (-) date delta = date + (negateDelta delta)
  
  // Delta literals
  1d = Day 1
  1m = Month 1
  1y = Year 1
  eom = End
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **isLeapYear** date<br><pre><code>`Date -> Bool`</code></pre> | Returns whether `date.year` is a leap year. |
| **daysInMonth** date<br><pre><code>`Date -> Int`</code></pre> | Returns the number of days in `date.month`. |
| **endOfMonth** date<br><pre><code>`Date -> Date`</code></pre> | Returns the last day of the month for `date`. |
| **addDays** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a day delta with calendar normalization. |
| **addMonths** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a month delta with normalization and day clamping. |
| **addYears** date n<br><pre><code>`Date -> Int -> Date`</code></pre> | Applies a year delta. |
| **negateDelta** delta<br><pre><code>`Delta -> Delta`</code></pre> | Returns the inverse delta (except `End`, which is idempotent). |

## Usage Examples

```aivi
use aivi.calendar

today = { year: 2025, month: 2, day: 8 }

tomorrow = today + 1d
nextMonth = today + 1m
lastYear = today - 1y
monthEnd = today + eom
```


<!-- FILE: /05_stdlib/02_chronos/03_duration -->

# Duration Domain

The `Duration` domain provides a type-safe way to represent **Spans of Time**.

In many systems, a timeout is just an integer like `500`. But is that 500 milliseconds? 500 seconds? Ambiguous units cause outages (like setting a 30-second timeout that the system reads as 30 milliseconds).

`Duration` solves this by wrapping the number in a type that knows its unit. `500` becomes `500ms` or `0.5s`. The compiler ensures you don't compare Seconds to Apples, stopping bugs before they start.

## Overview

```aivi
use aivi.duration (Duration)

// Clear, unambiguous literals
timeout = 500ms
delay = 2seconds

// Type-safe comparison
if delay > timeout {
    // ...
}
```

## Features

```aivi
Span = { millis: Int }
```

## Domain Definition

```aivi
domain Duration over Span = {
  type Delta = Millisecond Int | Second Int | Minute Int | Hour Int
  
  (+) : Span -> Delta -> Span
  (+) span (Millisecond n) = { millis: span.millis + n }
  (+) span (Second n)      = { millis: span.millis + n * 1000 }
  (+) span (Minute n)      = { millis: span.millis + n * 60000 }
  (+) span (Hour n)        = { millis: span.millis + n * 3600000 }
  
  (-) : Span -> Delta -> Span
  (-) span delta = span + (negateDelta delta)
  
  // Span arithmetic
  (+) : Span -> Span -> Span
  (+) s1 s2 = { millis: s1.millis + s2.millis }
  
  // Delta literals
  1ms = Millisecond 1
  1s = Second 1
  1min = Minute 1
  1h = Hour 1
}
```

## Usage Examples

```aivi
use aivi.duration

timeout = { millis: 0 } + 30s
delay = timeout + 500ms
longPoll = { millis: 0 } + 5min
```


<!-- FILE: /05_stdlib/03_system/20_file -->

# File Domain

The `File` domain bridges the gap between your code and the disk.

Your code lives in ephemeral memory, but data needs to persist. This domain lets you safely read configs, save user data, and inspect directories.
*   **Read/Write**: Load a config or save a savegame.
*   **Check**: "Does this file exist?"
*   **Inspect**: "When was this modified?"

Direct file access is dangerous (locks, missing files, permissions). AIVI wraps these in `Effect` types, forcing you to handle errors explicitly. Your program won't crash just because a file is missing; it will handle it.

## Overview

```aivi
use aivi.file (readText, stat)

// Safe reading
content = readText "config.json"

// Metadata inspection
match stat "large_video.mp4" {
    | Ok info => print "File size: {info.size} bytes"
    | Err _   => print "File not found"
}
```

## Types

```aivi
FileStats = {
  size: Int          // Size in bytes
  created: Int       // Unix timestamp (ms)
  modified: Int      // Unix timestamp (ms)
  isFile: Bool
  isDirectory: Bool
}
```

## Resource Operations

For more control or large files, use the resource-based API.

### `open`


| Function | Explanation |
| --- | --- |
| **open** path<br><pre><code>`String -> Effect (Resource Handle)`</code></pre> | Opens a file for reading and returns a managed `Handle` resource. |

### `readAll`


| Function | Explanation |
| --- | --- |
| **readAll** handle<br><pre><code>`Handle -> Effect (Result String Error)`</code></pre> | Reads the entire contents of an open handle as a string. |

### `close`


| Function | Explanation |
| --- | --- |
| **close** handle<br><pre><code>`Handle -> Effect Unit`</code></pre> | Closes the file handle (automatic with `resource` blocks). |

## Path Operations

### `readText`


| Function | Explanation |
| --- | --- |
| **readText** path<br><pre><code>`String -> Effect (Result String Error)`</code></pre> | Reads the entire contents of `path` as a string. |

### `writeText`


| Function | Explanation |
| --- | --- |
| **writeText** path contents<br><pre><code>`String -> String -> Effect (Result Unit Error)`</code></pre> | Writes `contents` to `path`, overwriting if it exists. |

### `exists`


| Function | Explanation |
| --- | --- |
| **exists** path<br><pre><code>`String -> Effect Bool`</code></pre> | Returns whether a file or directory exists at `path`. |

### `stat`


| Function | Explanation |
| --- | --- |
| **stat** path<br><pre><code>`String -> Effect (Result FileStats Error)`</code></pre> | Retrieves metadata about a file or directory at `path`. |

### `delete`


| Function | Explanation |
| --- | --- |
| **delete** path<br><pre><code>`String -> Effect (Result Unit Error)`</code></pre> | Removes the file at `path`. |


<!-- FILE: /05_stdlib/03_system/21_console -->

# Console Domain

The `Console` domain is your program's voice. It handles basic interactions with the terminal. Whether you're debugging with a quick `print`, logging a status update, or asking the user for input, this is where your program talks to the human running it.

```aivi
use aivi.console
```

## Functions

| Function | Explanation |
| --- | --- |
| **log** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` to standard output with a trailing newline. |
| **println** message<br><pre><code>`String -> Effect Unit`</code></pre> | Alias for `log`. |
| **print** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` without a trailing newline. |
| **error** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` to standard error. |
| **readLine** :()<br><pre><code>`Unit -> Effect (Result String Error)`</code></pre> | Reads a line from standard input. |


<!-- FILE: /05_stdlib/03_system/23_database -->

# Database Domain

The `Database` domain provides a type-safe, composable way to work with relational data. It treats tables as immutable sequences of records, while compiling predicates and patches into efficient SQL under the hood.

It builds on existing AIVI features:
- **Domains** for operator overloading and delta literals
- **Predicates** for filtering and joins
- **Patching** for declarative updates
- **Effects** for explicit error handling

## Overview

```aivi
use aivi.database as db

User = { id: Int, name: Text, email: Text?, active: Bool, loginCount: Int, createdAt: Instant }

@static
userTable : Table User
userTable = db.table "users" [
  { name: "id", type: IntType, constraints: [AutoIncrement, NotNull], default: None }
  { name: "name", type: Varchar 100, constraints: [NotNull], default: None }
  { name: "email", type: Varchar 255, constraints: [], default: None }
  { name: "active", type: BoolType, constraints: [NotNull], default: Some (DefaultBool True) }
  { name: "loginCount", type: IntType, constraints: [NotNull], default: Some (DefaultInt 0) }
  { name: "createdAt", type: TimestampType, constraints: [NotNull], default: Some DefaultNow }
]

getActiveUsers : Effect DbError (List User)
getActiveUsers = effect {
  users <- load userTable
  pure (users |> filter active |> sortBy .createdAt)
}
```

Table schemas are defined with ordinary values. `db.table` takes a table name and a
list of `Column` values; the row type comes from the table binding's type annotation.

## Types

```aivi
// Tables are sequences of rows
type Table A = List A

// Schema definitions are regular AIVI values.
// The row type is inferred from the table binding (e.g. Table User).
type ColumnType =
  | IntType
  | BoolType
  | TimestampType
  | Varchar Int

type ColumnConstraint =
  | AutoIncrement
  | NotNull

type ColumnDefault =
  | DefaultBool Bool
  | DefaultInt Int
  | DefaultText Text
  | DefaultNow

type Column = {
  name: Text
  type: ColumnType
  constraints: List ColumnConstraint
  default: ColumnDefault?
}

// Predicate alias
type Pred A = A => Bool

// Deltas express insert/update/delete
type Delta A =
  | Insert A
  | Update (Pred A) (Patch A)
  | Delete (Pred A)
```

## Domain Definition

```aivi
domain Database over Table A = {
  type Delta = Delta A

  (+) : Table A -> Delta A -> Effect DbError (Table A)
  (+) table delta = applyDeltaToDb table delta

  ins = Insert
  upd = Update
  del = Delete
}
```

### Applying Deltas

```aivi
createUser : User -> Effect DbError User
createUser newUser = effect {
  _ <- userTable + ins newUser
  pure newUser
}

activateUsers : Effect DbError Unit
activateUsers = effect {
  _ <- userTable + upd (!active) { active: True, loginCount: _ + 1 }
  pure Unit
}

deleteOldPosts : Instant -> Effect DbError Unit
deleteOldPosts cutoff = effect {
  _ <- postTable + del (_.createdAt < cutoff)
  pure Unit
}
```

## Querying

Tables behave like lazy sequences. Operations such as `filter`, `find`, `sortBy`, `groupBy`, and `join` build a query plan. The query executes only when observed (e.g. via `load`, `toList`, or a generator).

```aivi
getUserById : Int -> Effect DbError (Option User)
getUserById id = effect {
  users <- load userTable
  pure (users |> find (_.id == id))
}
```

## Joins and Preloading

```aivi
UserWithPosts = { user: User, posts: List Post }

getUsersWithPosts : Effect DbError (List UserWithPosts)
getUsersWithPosts = effect {
  users <- load userTable
  posts <- load postTable
  pure (
    users
    |> join posts on (_.id == _.authorId)
    |> groupBy { userId = _.id, user = _.left, post = _.right }
    |> map { key, group } => {
      user: group.head.user,
      posts: group |> map .post
    }
  )
}
```

For eager loading:

```aivi
usersWithPosts <- load (userTable |> preload posts on (_.id == _.authorId))
```

## Migrations

Schema definitions are typed values. Mark them `@static` to allow compile-time validation and migration planning.

```aivi
migrate : Effect DbError Unit
migrate = effect {
  _ <- db.runMigrations [ userTable ]
  pure Unit
}
```

## Notes

- `Database` compiles predicate expressions into `WHERE` clauses and patch instructions into `SET` clauses.
- Joins are translated into single SQL queries to avoid N+1 patterns.
- Advanced SQL remains available via `db.query` in [External Sources](../../02_syntax/12_external_sources.md).


<!-- FILE: /05_stdlib/03_system/25_url -->

# URL Domain

The `Url` domain handles **Uniform Resource Locators** without the string-mashing headaches.

A URL isn't just text; it's a structured address with protocols, hosts, and queries. Concatenating strings to build URLs leads to bugs (missing `/`, double `?`, unescaped spaces). This domain treats URLs as safe, structured records, letting you modify protocols or add query parameters without breaking the address.

## Module

```aivi
module aivi.url = {
  export domain Url
  export Url
  export parse, toString
}
```

## Types

```aivi
Url = {
  protocol: String,
  host: String,
  port: Option Int,
  path: String,
  query: List (String, String),
  hash: Option String
}
```

## Domain Definition

```aivi
domain Url over Url = {
  // Add a query parameter
  (+) : Url -> (String, String) -> Url
  (+) url (key, value) = { 
    ...url, 
    query: url.query ++ [(key, value)] 
  }
  
  // Remove a query parameter by key
  (-) : Url -> String -> Url
  (-) url key = { 
    ...url, 
    query: filter (\(k, _) -> k != key) url.query 
  }
  
  // Update record fields (standard record update syntax)
  // url <| { protocol: "https" }
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **parse** text<br><pre><code>`String -> Result Url Error`</code></pre> | Converts a URL string into a structured `Url`. |
| **toString** url<br><pre><code>`Url -> String`</code></pre> | Renders a `Url` back into its string form. |

## Usage Examples

```aivi
use aivi.url

// Create using the ~u sigil
base = ~u(https://api.example.com/v1/search)

// Add parameter: "?q=aivi"
search = base + ("q", "aivi")

// Add another: "?q=aivi&sort=desc"
sorted = search + ("sort", "desc")

// Change protocol or path using record update
secure_login = base <| { 
  path: "/v1/login",
  protocol: "wss" 
}
```


<!-- FILE: /05_stdlib/03_system/22_crypto -->

# Crypto Domain

The `Crypto` domain provides essential tools for security and uniqueness.

From generating unguessable **UUIDs** for database keys to hashing passwords with **SHA-256**, these functions ensure your program's sensitive data remains secure, unique, and tamper-evident.

```aivi
use aivi.crypto
```

## Functions

| Function | Explanation |
| --- | --- |
| **sha256** text<br><pre><code>`String -> String`</code></pre> | Returns the SHA-256 hash of `text` encoded as hex. |
| **randomUuid** :()<br><pre><code>`Unit -> Effect String`</code></pre> | Generates a random UUID v4. |
| **randomBytes** n<br><pre><code>`Int -> Effect Bytes`</code></pre> | Generates `n` random bytes. |


<!-- FILE: /05_stdlib/03_system/25_system -->

# System Domain

The `System` domain connects your program to the operating system.

It allows you to read **Environment Variables** (like secret queries or API keys), handle command-line arguments, or signal success/failure with exit codes. It is the bridge between the managed AIVI runtime and the chaotic host machine.

## Overview

```aivi
use aivi.system (Env)

// Read an environment variable
port = Env.get("PORT") |> Option.default("8080")
```

## Goals for v1.0

- Environment variables (read-only or read-write depending on capabilities).
- Command-line arguments.
- Process termination (`exit`).
- Spawning child processes (optional for v1.0, but good to plan).


<!-- FILE: /05_stdlib/03_system/26_log -->

# Log Domain

The `Log` domain provides **Structured Logging** for modern observability.

`print()` is fine for debugging, but production software needs data. This domain lets you attach metadata (like `{ userId: 123 }`) to your logs, making them machine-readable and ready for ingestion by tools like Datadog or Splunk.

## Overview

```aivi
use aivi.log (info, error)

info("Server started", { port: 8080, env: "prod" })
```

## Goals for v1.0

- Standard levels: `Trace`, `Debug`, `Info`, `Warn`, `Error`.
- Structured context (key-value pairs) rather than just format strings.
- Pluggable backends (console by default, WASI logging).


<!-- FILE: /05_stdlib/03_network/00_network -->

# Net Package

The `Net` package groups networking domains under a single entry point. These modules cover outbound HTTP/HTTPS, inbound servers, raw sockets, and stream utilities.

```aivi
use aivi.net
```

## Modules

- `aivi.net.http`
- `aivi.net.https`
- `aivi.net.http_server`
- `aivi.net.sockets`
- `aivi.net.streams`


<!-- FILE: /05_stdlib/03_network/01_http -->

# HTTP Domain

The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

```aivi
use aivi.net.http
```

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect (Result Response Error)`</code></pre> | Performs a GET request and returns a `Response` or `Error`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect (Result Response Error)`</code></pre> | Performs a POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect (Result Response Error)`</code></pre> | Performs a request with custom method, headers, and body. |

## Types

### `Response`

```aivi
type Response = {
    status: Int,
    headers: List { name: Text, value: Text },
    body: Text
}
```

### `Request`

```aivi
type Request = {
    method: Text,
    url: Url,
    headers: List { name: Text, value: Text },
    body: Option Text
}
```


<!-- FILE: /05_stdlib/03_network/02_https -->

# HTTPS Domain

The `Https` domain mirrors `Http`, but enforces secure (TLS) connections. It is intended for production use where transport security is required.

```aivi
use aivi.net.https
```

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect (Result Response Error)`</code></pre> | Performs a secure GET request and returns a `Response` or `Error`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect (Result Response Error)`</code></pre> | Performs a secure POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect (Result Response Error)`</code></pre> | Performs a secure request with custom method, headers, and body. |

## Types

Uses the same `Request` and `Response` types as `aivi.net.http`.


<!-- FILE: /05_stdlib/03_network/03_http_server -->

# HTTP Server Domain

The `HttpServer` domain provides a scalable HTTP/1.1 + HTTP/2 server with optional WebSocket upgrades. The server is designed to run across multiple CPU cores.

```aivi
use aivi.net.http_server
```

## Types

```aivi
type Header = { name: Text, value: Text }

type Request = {
  method: Text,
  path: Text,
  headers: List Header,
  body: List Int,
  remoteAddr: Option Text
}

type Response = {
  status: Int,
  headers: List Header,
  body: List Int
}

type ServerConfig = {
  address: Text
}

type HttpError = { message: Text }
type WsError = { message: Text }

type WsMessage
  = TextMsg Text
  | BinaryMsg (List Int)
  | Ping
  | Pong
  | Close

type ServerReply
  = Http Response
  | Ws (WebSocket -> Effect WsError Unit)
```

## Functions

| Function | Explanation |
| --- | --- |
| **listen** config handler<br><pre><code>`ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server`</code></pre> | Starts a server and yields a `Server` resource that stops on cleanup. |
| **stop** server<br><pre><code>`Server -> Effect HttpError Unit`</code></pre> | Stops a running server instance. |
| **wsRecv** socket<br><pre><code>`WebSocket -> Effect WsError WsMessage`</code></pre> | Receives the next WebSocket message. |
| **wsSend** socket message<br><pre><code>`WebSocket -> WsMessage -> Effect WsError Unit`</code></pre> | Sends a WebSocket message. |
| **wsClose** socket<br><pre><code>`WebSocket -> Effect WsError Unit`</code></pre> | Closes the WebSocket connection. |


<!-- FILE: /05_stdlib/03_network/04_sockets -->

# Sockets Domain

The `Sockets` domain exposes low-level TCP/UDP sockets for custom protocols and long-lived connections.

```aivi
use aivi.net.sockets
```

## Types

```aivi
type Address = { host: Text, port: Int }
type SocketError = { message: Text }
```

## TCP

| Function | Explanation |
| --- | --- |
| **listen** address<br><pre><code>`Address -> Resource Listener`</code></pre> | Creates a TCP listener bound to `address`. |
| **accept** listener<br><pre><code>`Listener -> Effect SocketError Connection`</code></pre> | Waits for and returns an incoming TCP connection. |
| **connect** address<br><pre><code>`Address -> Effect SocketError Connection`</code></pre> | Opens a TCP connection to `address`. |
| **send** connection bytes<br><pre><code>`Connection -> List Int -> Effect SocketError Unit`</code></pre> | Sends raw bytes to the remote endpoint. |
| **recv** connection<br><pre><code>`Connection -> Effect SocketError (List Int)`</code></pre> | Receives raw bytes from the remote endpoint. |
| **close** connection<br><pre><code>`Connection -> Effect SocketError Unit`</code></pre> | Closes the TCP connection. |


<!-- FILE: /05_stdlib/03_network/05_streams -->

# Streams Domain

The `Streams` domain provides stream-oriented utilities for processing inbound and outbound data without loading everything into memory.

```aivi
use aivi.net.streams
```

## Types

```aivi
type StreamError = { message: Text }
```

## Functions

| Function | Explanation |
| --- | --- |
| **fromSocket** connection<br><pre><code>`Connection -> Stream (List Int)`</code></pre> | Creates a stream of byte chunks from the connection. |
| **toSocket** connection stream<br><pre><code>`Connection -> Stream (List Int) -> Effect StreamError Unit`</code></pre> | Writes byte chunks from `stream` to the connection. |
| **chunks** size stream<br><pre><code>`Int -> Stream (List Int) -> Stream (List Int)`</code></pre> | Rechunks a byte stream into fixed-size blocks of `size`. |


<!-- FILE: /05_stdlib/04_ui/04_color -->

# Color Domain

The `Color` domain helps you work with **Colors** the way humans do.

Screens think in Red, Green, and Blue, but people think in **Hue**, **Saturation**, and **Lightness**. This domain lets you mix colors mathematically (e.g., `primary + 10% lightness` for a hover state) without the mud that comes from raw RGB math.

## Overview

```aivi
use aivi.color (Color)

primary = #007bff
// Mathematically correct lightening
lighter = primary + 10lightness
```

## Features

```aivi
Rgb = { r: Int, g: Int, b: Int }  // 0-255
Hsl = { h: Float, s: Float, l: Float }  // h: 0-360, s/l: 0-1
Hex = Text  // "#rrggbb"
```

## Domain Definition

```aivi
domain Color over Rgb = {
  type Delta = Lightness Int | Saturation Int | Hue Int
  
  (+) : Rgb -> Delta -> Rgb
  (+) col (Lightness n) = adjustLightness col n
  (+) col (Saturation n) = adjustSaturation col n
  (+) col (Hue n) = adjustHue col n
  
  (-) : Rgb -> Delta -> Rgb
  (-) col delta = col + (negateDelta delta)
  
  // Delta literals
  1l = Lightness 1
  1s = Saturation 1
  1h = Hue 1
}
```

## Helper Functions

| Function | Explanation |
| --- | --- |
| **adjustLightness** color amount<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Increases or decreases lightness by a percentage. |
| **adjustSaturation** color amount<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Increases or decreases saturation by a percentage. |
| **adjustHue** color degrees<br><pre><code>`Rgb -> Int -> Rgb`</code></pre> | Rotates hue by degrees. |
| **toRgb** hsl<br><pre><code>`Hsl -> Rgb`</code></pre> | Converts HSL to RGB. |
| **toHsl** rgb<br><pre><code>`Rgb -> Hsl`</code></pre> | Converts RGB to HSL. |
| **toHex** rgb<br><pre><code>`Rgb -> Hex`</code></pre> | Renders RGB as a hex string. |

## Usage Examples

```aivi
use aivi.color

primary = { r: 255, g: 85, b: 0 }  // Orange

lighter = primary + 10l
darker = primary - 20l
muted = primary - 30s
shifted = primary + 30h
```


<!-- FILE: /03_kernel/01_core_terms -->

# Core terms (expression kernel)

## 1.1 Variables

```text
x
```


## 1.2 Lambda abstraction (single-argument)

```text
λx. e
```

Multi-argument functions are **curried desugaring**.


## 1.3 Application

```text
e₁ e₂
```

Whitespace application is syntax only.


## 1.4 Let-binding

```text
let x = e₁ in e₂
```

All top-level and block bindings desugar to `let`.

## 1.4.1 Recursive let-binding

Recursion is required for practical programs (and is used throughout the spec examples). The kernel therefore includes a recursive binding form:

```text
let rec f = e₁ in e₂
```

Informally: `f` is in scope in both `e₁` and `e₂`.

An implementation may also support mutually-recursive groups as a convenience, but the kernel only needs a single-binder `let rec` as a primitive.


## 1.5 Algebraic data constructors

```text
C e₁ … eₙ
```

Nullary constructors are values.


## 1.6 Case analysis (single eliminator)

```text
case e of
  | p₁ → e₁
  | p₂ → e₂
```

This is the **only branching construct**.

* `?`
* multi-clause functions
* predicate patterns

all desugar to `case`.


<!-- FILE: /03_kernel/02_types -->

# Types (kernel)

## 2.1 Types

```text
τ ::= α | τ → τ | T τ₁ … τₙ
```


## 2.2 Universal quantification

```text
∀α. τ
```

This corresponds to `*` in surface syntax.


## 2.3 Row types (records)

```text
{ l₁ : τ₁, … | ρ }
```

* open records
* structural typing
* patching relies on this


<!-- FILE: /03_kernel/03_records -->

# Records (kernel)

## 3.1 Record construction

```text
{ l₁ = e₁, l₂ = e₂ }
```


## 3.2 Record projection

```text
e.l
```


## 3.3 Record update (primitive)

```text
update(e, l, f)
```

Semantics:

* apply `f` to field `l` if present
* otherwise insert if allowed by row type

> **This single primitive underlies patching**


<!-- FILE: /03_kernel/04_patterns -->

# Pattern binding (kernel)

## 4.1 Pattern forms

```text
p ::= x
    | C p₁ … pₙ
    | { l₁ = p₁, … }
```


## 4.2 Whole-value binding

```text
x @ p
```

This is **primitive**, not sugar.


<!-- FILE: /03_kernel/05_predicates -->

# Predicates (kernel)

There is **no predicate syntax** in the kernel.

A predicate is just:

```text
A → Bool
```

### Predicate sugar desugars to:

```text
λ_. e
```

Field shortcuts:

```text
price > 80
⇒ λx. x.price > 80
```

Pattern predicates:

```text
Some _
⇒ λx. case x of Some _ → True | _ → False
```


<!-- FILE: /03_kernel/06_traversals -->

# Traversals (kernel)

## 6.1 Fold (only traversal primitive)

```text
fold : ∀A B. (B → A → B) → B → List A → B
```

Everything else is built from this:

* `map`
* `filter`
* patch traversals
* generators


<!-- FILE: /03_kernel/07_generators -->

# Generators (kernel)

## 7.1 Generator as a Church-encoded fold

```text
Generator A ≡ ∀R. (R → A → R) → R → R
```

This means:

* generators are **just folds**
* no runtime suspension
* no special execution model

## 7.2 `yield`

```text
yield x ≡ λk acc. k acc x
```


<!-- FILE: /03_kernel/08_effects -->

# Effects (kernel)

## 8.1 Effect type

```text
Effect E A
```

Opaque in the kernel.


## 8.2 Effect bind

```text
bind : Effect E A → (A → Effect E B) → Effect E B
```

## 8.3 Effect pure / failure

```text
pure : A → Effect E A
fail : E → Effect E A
```


## 8.4 Effect sequencing

Everything desugars to `bind`.

No `do`, no `effect` in kernel.


<!-- FILE: /03_kernel/09_classes -->

# Classes (kernel)

## 9.1 Class = record of functions

```text
Class C τ ≡ { methods }
```

## 9.2 Instance = value

```text
instance : Class C τ
```

Resolution is **compile-time**, not runtime.


<!-- FILE: /03_kernel/10_domains -->

# Domains (kernel)

Domains are **not values**.

They are **static rewrite rules**:

```text
(operator, carrier-type) ↦ implementation
```

Example:

```text
(+, Date × MonthDelta) ↦ addMonth
```

This is **outside the term language**, like typing rules.


<!-- FILE: /03_kernel/11_patching -->

# Record patching (derived, not primitive)

A patch:

```text
x <| { a.b.c : f }
```

Desugars to nested `update` + `fold`:

```text
update x "a" (λa.
  update a "b" (λb.
    update b "c" f))
```

Predicates become `filter` over folds.

Removal is `update` to `None` + row shrink.


<!-- FILE: /03_kernel/12_minimality -->

# Minimality proof (informal)

| Feature | Kernel primitive |
| :--- | :--- |
| Lambdas | λ |
| Multi-arg functions | currying |
| Recursion | `let rec` |
| Patterns | case |
| `@` binding | primitive |
| Records | row types + update |
| Patching | update + fold |
| Predicates | λ + case |
| Generators | fold |
| Effects | bind |
| Domains | static rewrite |
| HKTs | ∀ |

Nothing else is required.


# The true kernel

> **AIVI’s kernel is simply:**
> **λ-calculus with algebraic data types, row-typed records with update, universal types, fold, and an opaque effect monad.**
> **Domains are static rewrite rules; patching, predicates, generators, and effects are all elaborations of these primitives.**


<!-- FILE: /04_desugaring/01_bindings -->

# Bindings, blocks, and shadowing

| Surface | Desugaring |
| :--- | :--- |
| `x = e` (top-level) | kernel `let rec x = ⟦e⟧ in …` (module elaboration; module-level bindings are recursive by default) |
| block: `f = a => b1 b2 b3` | `f = a => let _ = ⟦b1⟧ in let _ = ⟦b2⟧ in ⟦b3⟧` if `b1,b2` are effectless statements; if they are bindings, see next rows |
| block binding: `x = e` inside block | `let x = ⟦e⟧ in …` |
| shadowing: `x = 1; x = x + 1` | `let x = 1 in let x = x + 1 in …` |


<!-- FILE: /04_desugaring/02_functions -->

# Functions and application (comma-free)

| Surface | Desugaring |
| :--- | :--- |
| `x y => e` | `λx. λy. ⟦e⟧` |
| `x => e` | `λx. ⟦e⟧` |
| `f a b` | `⟦f⟧ ⟦a⟧ ⟦b⟧` (left-assoc) |
| `(e)` | `⟦e⟧` |


# Placeholder lambda `_`

`_` is only valid where a unary function is expected (syntactically or by typing). It desugars to a fresh binder.

| Surface | Desugaring |
| :--- | :--- |
| `_ + 1` (in lambda position) | `λx#1. x#1 + 1` |
| `toUpper` (value) | `toUpper` (no change) |


<!-- FILE: /04_desugaring/03_records -->

# Records: construction and projection

| Surface | Desugaring |
| :--- | :--- |
| `{ a: e1, b: e2 }` | `{ a = ⟦e1⟧, b = ⟦e2⟧ }` |
| `r.a` | `⟦r⟧.a` |
| `r.a.b` | `(⟦r⟧.a).b` |
| `r.a.b@{x}` | `⟦r.a.b⟧ { x }` (projection + binding) |


<!-- FILE: /04_desugaring/04_patterns -->

# Pattern binding with `=` (total-only)

Kernel has only `case`, so even total bindings can lower via `case`. (A compiler may optimize to projections.)

| Surface | Desugaring |
| :--- | :--- |
| `{ a: x } = e; body` | `case ⟦e⟧ of \| { a = x } -> ⟦body⟧` |
| `[h, ...t] = e; body` | `case ⟦e⟧ of \| (h :: t) -> ⟦body⟧` |
| `p = e; body` | `case ⟦e⟧ of \| ⟦p⟧ -> ⟦body⟧` |

### Deep Path Destructuring
| Surface | Desugaring |
| :--- | :--- |
| `{ a.b.c@{x} }` | `⟦{ a: { b: { c: v#1@{x} } } }⟧` |

Pattern translation `⟦p⟧` uses the kernel pattern forms.


# Whole-value binding `@`

| Surface | Desugaring |
| :--- | :--- |
| `v@p` (pattern) | kernel pattern `v @ ⟦p⟧` |
| `case e of \| v@{ name: n } -> b` | `case ⟦e⟧ of \| v @ { name = n } -> ⟦b⟧` |
| binding: `v@p = e; body` | `case ⟦e⟧ of \| v @ ⟦p⟧ -> ⟦body⟧` |


# Pattern matching `?`

Surface `?` is syntactic sugar for `case` with ordered arms.

| Surface | Desugaring |
| :--- | :--- |
| `e ? \| p1 => b1 \| p2 => b2` | `case ⟦e⟧ of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |
| guard: `\| p when g => b` | `\| ⟦p⟧ -> case ⟦g⟧ of \| True -> ⟦b⟧ \| False -> nextArm` (compiled as nested cases) |

Multi-clause functions:

| Surface | Desugaring |
| :--- | :--- |
| `f = \| p1 => b1 \| p2 => b2` | `f = λx#1. case x#1 of \| ⟦p1⟧ -> ⟦b1⟧ \| ⟦p2⟧ -> ⟦b2⟧` |


<!-- FILE: /04_desugaring/05_predicates -->

# Unified predicate expressions (for `filter`, path predicates, guards)

Predicate expression `pred` used where `A => Bool` expected:

| Surface | Desugaring |
| :--- | :--- |
| `filter (price > 80)` | `filter (λx#1. x#1.price > 80)` |
| `filter (_.price > 80)` | `filter (λx#1. x#1.price > 80)` |
| `filter (Some _)` | `filter (λx#1. case x#1 of \| Some _ -> True \| _ -> False)` |
| `items[price > 80]` (path segment) | traversal filter: `items[*]` + `filter` over element binding (see patch section) |

Rule (normative): inside predicate expressions, bare field `f` resolves to `_.f`.


<!-- FILE: /04_desugaring/06_generators -->

# Generators

## Generator core encoding

Generator type:

* `Generator A ≡ ∀R. (R -> A -> R) -> R -> R`

Primitive “constructors” as definable macros:

* `genEmpty = ΛR. λk. λz. z`
* `genYield a = ΛR. λk. λz. k z a`
* `genAppend g1 g2 = ΛR. λk. λz. g2 k (g1 k z)`
* `genMap f g = ΛR. λk. λz. g (λacc a. k acc (f a)) z`
* `genFilter p g = ΛR. λk. λz. g (λacc a. case p a of | True -> k acc a | False -> acc) z`

## `generate { … }`

| Surface | Desugaring |
| :--- | :--- |
| `generate { yield e }` | `genYield ⟦e⟧` |
| `generate { s1; s2 }` | `genAppend ⟦gen s1⟧ ⟦gen s2⟧` |
| `generate { x <- g; body }` | `genBind ⟦g⟧ (λx. ⟦generate { body }⟧)` where `genBind g f = ΛR. λk. λz. g (λacc a. (f a) k acc) z` |
| `generate { x -> pred; body }` | `genFilter (λx. ⟦pred⟧[_ := x]) ⟦generate { body }⟧` |
| `generate { loop pat = init => body }` | define local `recurse` and start it: `let rec recurse pat = ⟦generate { body }⟧ in recurse ⟦init⟧` |

## `resource { ... }`

Resources are desugared into `bracket` calls.

| Surface | Desugaring |
| :--- | :--- |
| `resource { setup; yield r; cleanup }` | `Resource { acquire = ⟦setup; pure r⟧, release = λr. ⟦cleanup⟧ }` |

(Note: This is a simplification; the actual desugaring handles the `Resource` type wrapper).


<!-- FILE: /04_desugaring/07_effects -->

# Effects: `effect` block

Kernel effect primitives:

* `pure : A -> Effect E A`
* `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
* `fail : E -> Effect E A`

## `effect { … }`

`effect` is the same pattern but over `Effect` with `bind/pure`:

| Surface | Desugaring |
| :--- | :--- |
| `effect { x <- e; body }` | `bind ⟦e⟧ (λx. ⟦effect { body }⟧)` |
| `effect { x = e; body }` | `let x = ⟦e⟧ in ⟦effect { body }⟧` |
| `effect { e; body }` | `bind ⟦e⟧ (λ_. ⟦effect { body }⟧)` (if `e : Effect`) |
| `effect { e }` | `⟦e⟧` (the final expression must already be an `Effect`) |
| `effect { }` | `pure Unit` |
| `effect { s1; ...; sn }` (no final expression) | `⟦effect { s1; ...; sn; pure Unit }⟧` |

If you want to return a pure value from an effect block, write `pure value` as the final expression.

If the surface allows `print` etc as effectful calls, those are already `Effect`-typed; no special desugaring beyond `bind`.


<!-- FILE: /04_desugaring/08_classes -->

# Classes and instances

Classes elaborate to records of methods (dictionary passing is compile-time, but can be expressed in kernel).

| Surface | Desugaring |
| :--- | :--- |
| `class Functor (F *) = { map: ... }` | type-level: `FunctorDict F = { map : ∀A B. F A -> (A -> B) -> F B }` |
| `instance Monad (Option *) = { ... }` | value-level: `monadOption : MonadDict Option = { ... }` |
| method call `map xs f` | `map{dict} xs f` after resolution (or `dict.map xs f`) |

(Resolution/elaboration is a compile-time phase; kernel representation is dictionary passing.)


<!-- FILE: /04_desugaring/09_domains -->

# Domains and operator resolution

Domains are not terms; they elaborate operator syntax to named functions.

| Surface | Desugaring |
| :--- | :--- |
| `a + b` | `(+)_D ⟦a⟧ ⟦b⟧` where `D` is the resolved domain for the carrier of `a` |
| `date + 1m` | `addMonth date 1m` (or domain-specific `applyDelta`) |
| `col + 3l` | `applyLightness col 3l` |

This is a static rewrite: `(operator, carrier-type)` ↦ implementation.


## 9.1 Delta Literal Resolution

Delta literals are **domain-scoped**. Resolution follows a two-step process:

| Step | Action | Example |
| :--- | :--- | :--- |
| 1. Lexical lookup | Find delta binding in used domains | `1m` → defined in Calendar, Physics |
| 2. Carrier disambiguation | Select domain matching operand type | `date + 1m` → Calendar (date : Date) |

### Resolution Chain

```text
date + 1m
  ↓ (step 1: find delta)
  1m is defined in: Calendar.Delta.Month, Physics.Delta.Meter
  ↓ (step 2: carrier type)
  date : Date → Calendar domain
  ↓ (step 3: expand delta)
  date + (Month 1)
  ↓ (step 4: resolve operator)
  Calendar.(+) date (Month 1)
  ↓ (step 5: desugar to implementation)
  addMonth date (Month 1)
```


## 9.2 Ambiguity Errors

When carrier type cannot disambiguate:

```aivi
x + 1m  // Error: x : Int, neither Calendar nor Physics apply
```

When multiple domains match:

```aivi
// If both Calendar and Physics define (+) over the same carrier
ambiguous + 1m  // Error: Ambiguous domain for (+)
```

Resolution: Use qualified literals or operators.

```aivi
date + Calendar.1m
position + Physics.1m
```


## 9.3 Operator Precedence

Domain operators follow standard precedence. Domains do not redefine precedence — only semantics:

```aivi
1 + 2 * 3      // Parsed as: 1 + (2 * 3)
date + 1m      // Parsed as: (date + 1m)
```


## 9.4 Desugaring Order

1. **Type inference** — Determine carrier types
2. **Delta expansion** — Replace literals with constructors
3. **Domain resolution** — Match (operator, carrier) to domain
4. **Function substitution** — Replace operator with implementation


<!-- FILE: /04_desugaring/10_patching -->

# Record patching `<|` (Path : Instruction)

Kernel record primitives:

* `update(e, l, f)` : update/insert field `l` by applying `f` to old value (or a sentinel for missing)
* field removal is a **typing/elaboration** operation (row shrink) plus a runtime representation choice; a compiler may lower `-` either to a dedicated `delete(e, l)` primitive or to an `update` that drops the field in a representation-specific way.

For nested paths, desugar into nested `update`/`delete`.

## Path compilation (dot paths)

| Surface | Desugaring |
| :--- | :--- |
| `r <| { a: v }` | `update ⟦r⟧ "a" (λ_. ⟦v⟧)` (replace/insert) |
| `r <| { a: f }` where `f` is a function | `update ⟦r⟧ "a" ⟦f⟧` (transform) |
| `r <| { a: - }` | `removeField ⟦r⟧ "a"` (derived; shrinks row type) |

Nested:

| Surface | Desugaring |
| :--- | :--- |
| `r <| { a.b: v }` | `update ⟦r⟧ "a" (λa0. update a0 "b" (λ_. ⟦v⟧))` |
| `r <| { a.b: f }` | `update ⟦r⟧ "a" (λa0. update a0 "b" ⟦f⟧)` |
| `r <| { a.b: - }` | `update ⟦r⟧ "a" (λa0. removeField a0 "b")` |

## Function-as-data disambiguation `:=`

| Surface | Desugaring |
| :--- | :--- |
| `path: := (λx. e)` | `update carrier path (λ_. (λx. ⟦e⟧))` and mark as “value replacement” (no transform) |

Formally:

* `path: f` (function) → transform
* `path: := f` → replace with function value

## Automatic lifting for patch instructions

Patch instruction `instr` is lifted when the targeted field type is `Option T` or `Result E T`.

Let `L(instr)` be the lifted instruction:

* If field is `T`: apply instruction normally.
* If field is `Option T`: `mapOption instr`
* If field is `Result E T`: `mapResult instr`

Desugaring (conceptual) for transform instruction `f`:

* `Option`: `λopt. case opt of \| Some x -> Some (f x) \| None -> None`
* `Result`: `λres. case res of \| Ok x -> Ok (f x) \| Err e -> Err e`

Replacement (`value`) replaces the whole container unless explicitly targeted deeper (e.g. `.Some.val`).

## Traversals `[*]`

Path segment `items[*].price: f` desugars to `map` over list plus nested patch.

| Surface | Desugaring |
| :--- | :--- |
| `items[*].price: f` | `update r "items" (λxs. map (λit. update it "price" ⟦f⟧) xs)` |

## Predicate traversal `items[pred]`

Predicate segments desugar to **map with conditional update**.

| Surface | Desugaring |
| :--- | :--- |
| `items[pred].price: f` | `update r "items" (λxs. map (λit. case (⟦pred→λ⟧ it) of \| True -> update it "price" ⟦f⟧ \| False -> it) xs)` |

Predicate `pred` uses the unified predicate desugaring table (Section 8).

## Sum-type focus (prisms)

`Ok.value: f` desugars to a constructor check and selective update.

| Surface | Desugaring |
| :--- | :--- |
| `Ok.value: f` | `λres. case res of \| Ok v -> Ok (update v "value" ⟦f⟧) \| _ -> res` (record payload) |
| `Some.val: f` | `λopt. case opt of \| Some v -> Some (update v "val" ⟦f⟧) \| _ -> opt` |

For constructors with direct payload (not record), `value` refers to the payload position.

## Map key selectors

When a path segment selects a `Map` entry, desugar to `Map` operations. The selector focuses on the **value**.

Assume `m : Map K V` and `k : K`:

| Surface | Desugaring |
| :--- | :--- |
| `m <| { ["k"]: v }` | `Map.insert "k" ⟦v⟧ ⟦m⟧` |
| `m <| { ["k"]: f }` | `Map.update "k" ⟦f⟧ ⟦m⟧` |
| `m <| { ["k"]: - }` | `Map.remove "k" ⟦m⟧` |
| `m <| { ["k"].path: f }` | `Map.update "k" (λv0. update v0 "path" ⟦f⟧) ⟦m⟧` |

### Map traversal `map[*]`

| Surface | Desugaring |
| :--- | :--- |
| `map[*].path: f` | `Map.map (λv0. update v0 "path" ⟦f⟧) ⟦map⟧` |

### Map predicate traversal `map[pred]`

Predicate `pred` is applied to an entry record `{ key, value }`.

| Surface | Desugaring |
| :--- | :--- |
| `map[pred].path: f` | `Map.mapWithKey (λk v. case (⟦pred→λ⟧ { key: k, value: v }) of \| True -> update v "path" ⟦f⟧ \| False -> v) ⟦map⟧` |

# Summary: smallest set of kernel primitives assumed

* `λ`, application
* `let`
* `case` + patterns (including `@`)
* ADT constructors
* records + projection + `update` + `delete`
* `fold` (List)
* `Effect` with `bind/pure`
* compile-time elaboration for classes (dictionary passing)
* compile-time rewrite for domains (operator resolution)


<!-- FILE: /06_runtime/01_concurrency -->

# Runtime: Concurrency and Communication

AIVI implements a **Structural Concurrency** model by default, ensuring that the lifecycle of concurrent tasks is strictly bound to the lexical scope that created them.


## 20.1 Structural Concurrency

Structural concurrency means: concurrent tasks are children of the scope that spawned them. When the scope ends, all children have either completed or are cancelled (with cleanup).

### Primitives

For parser simplicity in v0.1, these are described as **standard library APIs** (taking thunks / effects), even if future surface syntax adds dedicated blocks:

- `concurrent.scope : Effect E A -> Effect E A`
- `concurrent.par   : Effect E A -> Effect E B -> Effect E (A, B)`
- `concurrent.race  : Effect E A -> Effect E A -> Effect E A`

### Explicit Detachment

When a task must outlive its creator (e.g., a background daemon), it must be explicitly detached from the structural tree.

```aivi
effect {
  _ <- concurrent.spawnDetached logger.run
  pure Unit
}
```


## 20.2 Communication: Channels

AIVI uses typed CSP-style channels for communication between concurrent tasks.

### Types

```aivi
Send A // Capability to send values of type A
Recv A // Capability to receive values of type A
```

### Channel Operations

```aivi
effect {
  (tx, rx) = channel.make ()
  
  // Sending
  _ <- channel.send tx "hello"
  
  // Receiving (returns Result for closed channels)
  res <- channel.recv rx
  msg = res ?
    | Ok value     => value
    | Err Closed   => "Channel closed"
  
  // Closing
  _ <- channel.close tx
  pure Unit
}
```


## 20.3 Non-deterministic Selection (select)

Selecting across multiple concurrent operations is essential for channel-based code.

```aivi
// Proposed surface syntax (future):
// next = select {
//   rx1.recv () => msg => handle1 msg
//   rx2.recv () => msg => handle2 msg
//   timeout 1s  => _   => handleTimeout ()
// }
```

The first operation to succeed is chosen; all other pending operations in the block are cancelled.


# PART 2: EXAMPLES



<!-- EXAMPLE: 00_literals.aivi -->

```aivi
module examples.compiler.literals = {
  export ints, floats, texts, suffixes, instant, tuple, records, nested, palette

  ints = [0, 1, 42, -7]
  floats = [0.0, 3.14, -2.5]
  texts = ["plain", "Count: {1 + 2}", "user: { { name: \"A\" }.name }"]

  suffixes = [10px, 100%, 30s, 1min, 3.14dec, 42n]
  instant = 2024-05-21T12:00:00Z

  tuple = (1, "ok", True)

  records = [
    { id: 1, label: "alpha", meta: { score: 9.5, active: True } }
    { id: 2, label: "beta", meta: { score: 7.0, active: False } }
  ]

  nested = {
    title: "Report"
    stats: { count: 3, avg: 1.5 }
    tags: ["a", "b", "c"]
  }

  palette = [
    { name: "ink", rgb: (12, 15, 20) }
    { name: "sand", rgb: (242, 233, 210) }
  ]
}

```


<!-- EXAMPLE: 01_patterns.aivi -->

```aivi
module examples.compiler.patterns = {
  export head, classify, deepName, unwrap, take, zip, depth, flatten, evalExpr

  Option A = None | Some A
  Result E A = Err E | Ok A
  Tree A = Leaf A | Node (Tree A) (Tree A)
  Expr = Num Int | Add Expr Expr | Mul Expr Expr

  head =
    | [] => None
    | [x, ...] => Some x

  classify = n => n ?
    | 0 => "zero"
    | n when n < 0 => "negative"
    | _ => "positive"

  deepName = response =>
    response ?
      | { data.user.profile@{ name } } => name
      | _ => "unknown"

  unwrap =
    | Ok x => x
    | Err _ => 0

  take = (n, xs) => (n, xs) ?
    | (n, _) when n <= 0 => []
    | (_, []) => []
    | (n, [x, ...rest]) => [x, ...take (n - 1, rest)]

  zip = (xs, ys) => (xs, ys) ?
    | ([], _) => []
    | (_, []) => []
    | ([x, ...xs], [y, ...ys]) => [(x, y), ...zip (xs, ys)]

  append = xs ys => xs ?
    | [] => ys
    | [h, ...t] => [h, ...append t ys]

  depth =
    | Leaf _ => 1
    | Node left right => 1 + max (depth left) (depth right)

  flatten =
    | Leaf x => [x]
    | Node left right => append (flatten left) (flatten right)

  evalExpr =
    | Num n => n
    | Add a b => evalExpr a + evalExpr b
    | Mul a b => evalExpr a * evalExpr b
}

```


<!-- EXAMPLE: 02_functions.aivi -->

```aivi
module examples.compiler.functions = {
  export inc, add, compose, pipeline, getName, pairSum, twice, map, filter, sum, processed

  inc = _ + 1
  add = x y => x + y
  compose = f g => x => f (g x)

  pipeline = x => x |> inc |> (_ * 2)
  getName = .name
  pairSum = (a, b) => a + b

  twice = f => x => f (f x)

  map = f xs => xs ?
    | [] => []
    | [h, ...t] => [f h, ...map f t]

  filter = p xs => xs ?
    | [] => []
    | [h, ...t] when p h => [h, ...filter p t]
    | [_, ...t] => filter p t

  sum = xs => xs ?
    | [] => 0
    | [h, ...t] => h + sum t

  processed =
    [1, 2, 3, 4, 5]
      |> map (add 10)
      |> filter (_ % 2 == 0)
      |> sum
}

```


<!-- EXAMPLE: 03_records_patching.aivi -->

```aivi
module examples.compiler.records_patching = {
  export User, user1, user2, user3, store2, store3, promote

  User = { name: Text, age: Int, tags: List Text }

  append = xs ys => xs ?
    | [] => ys
    | [h, ...t] => [h, ...append t ys]

  user1 : User
  user1 = { name: "Ada", age: 36, tags: ["dev"] }

  user2 = user1 <| {
    age: _ + 1
    tags: append _ ["vip"]
  }

  promote = user => user <| {
    tags: append _ ["core"]
  }

  bumpName = name => "{name}+"

  user3 = user2 <| {
    name: bumpName
    age: _ + 2
  }

  Item = { price: Int, active: Bool }
  Store = { items: List Item }

  store = { items: [
    { price: 10, active: True }
    { price: 20, active: False }
    { price: 30, active: True }
  ] }

  store2 = store <| {
    items[active].price: _ + 5
  }

  store3 = store2 <| {
    items[price > 15].price: _ - 2
    items[price > 15].active: True
  }
}

```


<!-- EXAMPLE: 04_generators.aivi -->

```aivi
module examples.compiler.generators = {
  export gen, evens, grid, fibs, pairs, squares

  gen = generate {
    yield 1
    yield 2
    yield 3
  }

  evens = generate {
    x <- [1..10]
    x -> _ % 2 == 0
    yield x
  }

  grid = generate {
    x <- [0..2]
    y <- [0..3]
    yield (x, y)
  }

  pairs = generate {
    x <- [1..5]
    y <- [1..5]
    y -> y > x
    yield { left: x, right: y, sum: x + y }
  }

  squares = generate {
    x <- [1..8]
    sq = x * x
    sq -> _ % 2 == 0
    yield sq
  }

  fibs = generate {
    loop (a, b) = (0, 1) => {
      yield a
      recurse (b, a + b)
    }
  }
}

```


<!-- EXAMPLE: 05_effects_resources.aivi -->

```aivi
module examples.compiler.effects = {
  export program, readConfig, withFile, managedFile, loadOrDefault, program2

  use aivi.console (print)

  program : Effect Text Unit
  program = effect {
    _ <- print "boot"
    cfg <- readConfig "config.json"
    _ <- print cfg
    pure Unit
  }

  readConfig : Text -> Effect Text Text
  readConfig path = effect {
    res <- attempt (load (file.read path))
    res ?
      | Ok txt => pure txt
      | Err _  => fail "missing"
  }

  managedFile : Text -> Resource Text
  managedFile path = resource {
    handle <- file.open path
    yield handle
    _ <- file.close handle
  }

  withFile : Text -> Effect Text Unit
  withFile path = effect {
    f <- managedFile path
    _ <- file.readAll f
    pure Unit
  }

  loadOrDefault : Text -> Text -> Effect Text Text
  loadOrDefault path fallback = effect {
    res <- attempt (load (file.read path))
    res ?
      | Ok txt => pure txt
      | Err _  => pure fallback
  }

  program2 : Effect Text Unit
  program2 = effect {
    cfg <- loadOrDefault "config.json" "{ \"mode\": \"dev\" }"
    _ <- print cfg
    pure Unit
  }
}

```


<!-- EXAMPLE: 06_domains.aivi -->

```aivi
module examples.compiler.domains = {
  export Date, Calendar, addWeek, addTwoWeeks, shiftBy

  Date = { year: Int, month: Int, day: Int }

  domain Calendar over Date = {
    type Delta = Day Int | Week Int

    (+) : Date -> Delta -> Date
    (+) d (Day n) = addDays d n
    (+) d (Week n) = addDays d (n * 7)

    1d = Day 1
    1w = Week 1
  }

  addDays : Date -> Int -> Date
  addDays d n = d <| { day: _ + n }

  addWeek : Date -> Date
  addWeek d = d + 1w

  addTwoWeeks : Date -> Date
  addTwoWeeks d = d + 1w + 1w

  shiftBy : Date -> Delta -> Date
  shiftBy d delta = d + delta
}

```


<!-- EXAMPLE: 07_modules.aivi -->

```aivi
@no_prelude
module examples.compiler.math = {
  export add, sub, mul, square

  add = x y => x + y
  sub = x y => x - y
  mul = x y => x * y
  square = x => mul x x
}

module examples.compiler.stats = {
  export sum, mean

  use examples.compiler.math (add)

  sum = xs => xs ?
    | [] => 0
    | [h, ...t] => add h (sum t)

  sumCount = xs => xs ?
    | [] => (0, 0)
    | [h, ...t] => (sumCount t) ?
      | (s, n) => (h + s, n + 1)

  mean = xs => (sumCount xs) ?
    | (_, n) when n == 0 => 0
    | (s, n) => s / n
}

module examples.compiler.app = {
  export run

  use examples.compiler.math (add, square)
  use examples.compiler.stats (mean)

  run = [1, 2, 3, 4]
    |> mean
    |> add (square 2)
}

```


<!-- EXAMPLE: 08_effects_core_ops.aivi -->

```aivi
module examples.runtime_effects_core_ops = {
  export main

  use aivi.console (print)

  main : Effect Text Unit
  main = effect {
    n <- pure 41
    m <- (pure n |> bind) (x => pure (x + 1))

    res <- attempt (
      if m == 42 then fail "boom" else pure m
    )

    verdict <- res ?
      | Ok _  => pure "ok"
      | Err _ => pure "err"

    _ <- print verdict

    _ <- if m > 40 then effect {
      _ <- print "branch"
      pure Unit
    } else pure Unit

    pure Unit
  }
}

```


<!-- EXAMPLE: 09_classes.aivi -->

```aivi
module examples.compiler.classes = {
  export Eq, Functor, Option, isNone

  class Eq A = {
    eq: A -> A -> Bool
  }

  instance Eq Bool = {
    eq: x y => x == y
  }

  Option A = None | Some A

  class Functor (F *) = {
    map: F A -> (A -> B) -> F B
  }

  instance Functor (Option *) = {
    map: opt f => opt ?
      | None => None
      | Some x => Some (f x)
  }

  instance Eq (Option Bool) = {
    eq: a b => (a, b) ?
      | (None, None) => True
      | (Some x, Some y) => x == y
      | _ => False
  }

  isNone = opt => opt ?
    | None => True
    | Some _ => False
}

```


<!-- EXAMPLE: 11_concurrency.aivi -->

```aivi
module examples.runtime = {
  export main

  use aivi.console (print)

  main : Effect Text Unit
  main = effect {
    _ <- concurrent.par (print "left\n") (print "right\n")
    (tx, rx) <- channel.make Unit
    prefix = "ping"
    _ <- channel.send tx "{prefix}\n"
    res <- channel.recv rx
    msg <- res ?
      | Ok text => pure text
      | Err Closed => pure "closed\n"
    _ <- print msg
    pure Unit
  }
}

```


<!-- EXAMPLE: 12_text_regex.aivi -->

```aivi
module examples.stdlib_text_regex = {
  export main

  use aivi.console (print)
  use aivi.text
  use aivi.regex

  main : Effect Text Unit
  main = effect {
    msg = "  Hello World  "
    slug = msg |> trim |> toLower |> replaceAll " " "-"
    _ <- print slug

    compiled = compile "[a-z]+"
    verdict = compiled ?
      | Ok r => if test r "caa" then "match" else "no"
      | Err _ => "bad"

    _ <- print verdict

    bytes = toBytes Utf8 "ping"
    decoded = fromBytes Utf8 bytes
    decoded ?
      | Ok value => print value
      | Err _ => print "decode failed"

    pure Unit
  }
}

```


<!-- EXAMPLE: 13_calendar_color.aivi -->

```aivi
module examples.stdlib_calendar_color = {
  export main

  use aivi.console (print)
  use aivi.calendar
  use aivi.duration
  use aivi.color
  use aivi.vector

  main : Effect Text Unit
  main = effect {
    today = { year: 2025, month: 2, day: 8 }
    next = today + Day 1
    _ <- print "{next.year}-{next.month}-{next.day}"

    timeout = { millis: 0 } + Second 1
    _ <- print "timeout: {timeout.millis}"

    primary = { r: 10, g: 20, b: 30 }
    lighter = primary + Lightness 10
    _ <- print (toHex lighter)

    v1 = { x: 1.0, y: 2.0 }
    v2 = { x: 3.0, y: 4.0 }
    v3 = v1 + v2
    _ <- print "{v3.x},{v3.y}"

    pure Unit
  }
}

```


<!-- EXAMPLE: 14_math_number.aivi -->

```aivi
module examples.stdlib_math_number = {
  export main

  use aivi.console (print)
  use aivi.math
  use aivi.number.bigint (fromInt)
  use aivi.number.decimal (fromFloat)

  main : Effect Text Unit
  main = effect {
    g = gcd 54 24
    _ <- print "gcd: {g}"

    fact = factorial 10
    _ <- print "10! = {fact}"

    price = fromFloat 19.99
    total = price + fromFloat 0.01
    _ <- print "total: {total}"

    unit = sin (degrees 90.0)
    _ <- print "sin: {unit}"

    pure Unit
  }
}

```


<!-- EXAMPLE: 15_http_client.aivi -->

```aivi
module examples.httpClient = {
  export main

  use aivi.console (println)
  use aivi.net.https (get)

  main : Effect Text Unit
  main = effect {
    result <- get ~u(https://example.com)
    message = result ?
      | Ok response => "Status {response.status}"
      | Err err => "Error {err.message}"
    _ <- println message
    pure Unit
  }
}

```


<!-- EXAMPLE: 16_collections.aivi -->

```aivi
module examples.stdlib_collections = {
  export main

  use aivi.console (print)
  use aivi (Map, Set, Queue, Deque, Heap)

  main : Effect Text Unit
  main = effect {
    base = ~map{
      "a" => 1
      "b" => 2
    }
    more = ~map{
      "b" => 3
      "c" => 4
    }
    merged = Map.union base more
    _ <- print "merged: {merged}"

    tags = ~set["hot", "new", ...Set.fromList ["fresh"]]
    _ <- print "tags: {tags}"

    q1 = Queue.enqueue "first" Queue.empty
    q2 = Queue.enqueue "second" q1
    popped = Queue.dequeue q2
    popped ?
      | Some (value, _) => print "queue: {value}"
      | None => print "queue empty"

    d1 = Deque.pushFront 1 Deque.empty
    d2 = Deque.pushBack 2 d1
    d3 = Deque.pushBack 3 d2
    front = Deque.popFront d3
    front ?
      | Some (value, _) => print "deque: {value}"
      | None => print "deque empty"

    h1 = Heap.push 3 Heap.empty
    h2 = Heap.push 1 h1
    h3 = Heap.push 2 h2
    smallest = Heap.popMin h3
    smallest ?
      | Some (value, _) => print "heap: {value}"
      | None => print "heap empty"

    pure Unit
  }
}

```


<!-- EXAMPLE: 17_linear_signal_graph.aivi -->

```aivi
module examples.stdlib_linear_signal_graph = {
  export main

  use aivi.console (print)
  use aivi.linear_algebra
  use aivi.signal
  use aivi.graph

  main : Effect Text Unit
  main = effect {
    v1 = { size: 3, data: [1.0, 2.0, 3.0] }
    v2 = { size: 3, data: [2.0, 0.0, 1.0] }
    d = dot v1 v2
    _ <- print "dot: {d}"

    m1 = { rows: 2, cols: 2, data: [1.0, 2.0, 3.0, 4.0] }
    m2 = { rows: 2, cols: 2, data: [2.0, 0.0, 1.0, 2.0] }
    m3 = matMul m1 m2
    _ <- print "matMul: {m3}"

    sig = { samples: [0.0, 1.0, 0.0, -1.0], rate: 4.0 }
    windowed = windowHann sig
    _ <- print "windowed: {windowed.samples}"

    g0 = {
      nodes: [1, 2, 3],
      edges: [
        { from: 1, to: 2, weight: 1.0 },
        { from: 2, to: 3, weight: 1.0 },
        { from: 1, to: 3, weight: 5.0 }
      ]
    }
    path = shortestPath g0 1 3
    _ <- print "path: {path}"

    pure Unit
  }
}

```


<!-- EXAMPLE: hello.aivi -->

```aivi
module examples.hello = {
  export main

  use aivi.console (print)

  main : Effect Text Unit
  main = effect {
    _ <- print "Hello, world!"
    pure Unit
  }
}

```
