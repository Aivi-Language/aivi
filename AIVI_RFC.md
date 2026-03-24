# AIVI Language Specification

## Draft v0.7 — implementation-facing resolved pass

> Status: normative working draft with implementation choices merged. Later implementation decisions override earlier exploratory notes, and this pass resolves the recorded choice log into the RFC text. Milestones 1–8 (surface through backend) are substantially complete. Sections §26–§28 cover the CLI, LSP, and pre-stdlib implementation gaps.

---

## 1. Vision

AIVI is a purely functional, reactive, GTK/libadwaita-first programming language for building native Linux desktop applications.

Its defining shape is:

- pure user code by default
- strict closed types
- no `null` / `undefined`
- no `if` / `else`
- no loops in the traditional sense in the surface language
- expression-first control flow
- pipe algebra as a first-class surface
- first-class signals and source-backed reactivity
- higher-kinded abstractions in the core
- native compilation through Rust and Cranelift
- a runtime that integrates scheduler, signal propagation, GC, sources, and GTK

AIVI is not a thin syntax layer over Rust or GTK. It is a coherent language with a pure semantic core and an explicit runtime boundary.

---

## 2. Design goals and non-goals

### 2.1 Primary goals

- Make GTK4/libadwaita application development on GNOME Linux the flagship use case.
- Preserve a pure, explicit, analyzable user model.
- Make reactivity part of the language, not an afterthought library.
- Compile to native code JIT/AOT.
- Keep correctness legible through closed types, explicit boundaries, and strong diagnostics.

### 2.2 Non-goals for v1

The initial implementation does **not** optimize for:

- unrestricted systems programming
- implicit mutation-oriented UI models
- open-world structural typing
- type-level metaprogramming beyond narrow HKT support
- general-purpose dynamic graph monads for signals

These are non-goals for v1 because they weaken the main design: a pure, typed, reactive language for native desktop software.

---

## 3. Implementation invariants

This section is normative for the implementation architecture.

### 3.1 Semantic invariants

- Ordinary user functions are pure.
- `Signal` values denote time-varying values whose dependencies are known after elaboration.
- `Task E A` denotes a one-shot effectful computation description; it is not an immediate effect.
- Closed records reject undeclared fields.
- Closed sum types have a finite known constructor set.
- Pattern matching on sums is exhaustiveness-checked.

### 3.2 Ownership invariants

- Ordinary AIVI values are runtime-managed and may move.
- Stable addresses are not guaranteed for ordinary values.
- Stable foreign-facing identity is provided through runtime handles, pinned wrappers, or copied boundary values.
- GTK widgets and foreign runtime objects are never exposed as ordinary moving AIVI values.

### 3.3 Threading invariants

- GTK widget creation, mutation, and event dispatch are confined to the GTK main thread.
- Workers never mutate UI-owned state directly.
- Cross-thread communication is message-based and immutable from the user model.
- Scheduler ticks are single-owner operations from the runtime's point of view.

### 3.4 Stack-safety invariants

- No implementation pass may rely on unbounded Rust recursion over user-controlled depth.
- Tail recursion in lowered runtime code must be compiled in a stack-safe form.
- Signal propagation, pattern compilation, decode walking, and tree traversals must use explicit worklists or bounded recursion strategies where input depth is unbounded.
- The implementation must include deep-input torture tests.

### 3.5 IR invariants

Each IR boundary must define:

- node ownership model
- identity strategy
- source span strategy
- validation rules and entry points
- pretty-print/debug output
- losslessness expectations when the layer claims source fidelity

### 3.6 Error-reporting invariants

- Diagnostics are attached to source spans and preserve the user's surface constructs where possible.
- Desugaring must not erase the ability to point at the original cause.
- Ambiguity is surfaced explicitly rather than guessed silently.

---

## 4. Compiler pipeline

The implementation pipeline is:

1. **Lexer / parser**
2. **CST**
3. **HIR**
4. **Typed core**
5. **Closed typed lambda IR**
6. **Backend IR**
7. **Cranelift code generation**
8. **Runtime integration**

The repository keeps the implementation-facing companion contract for these layers in
`docs/ir-boundary-contracts.md`. The RFC freezes the minimum semantics each boundary must
preserve.

### 4.1 CST

The CST is source-oriented and lossless enough for formatting and diagnostics.

Boundary contract:

- ownership: `aivi_syntax::ParsedModule` owns both the lossless token buffer and the structural
  CST module
- identity: top-level items are source-addressed by `TokenRange` into the token buffer rather than
  synthetic arena ids; nested nodes are structural within their parent item
- source spans: user-addressable CST nodes carry `SourceSpan`; top-level items additionally retain
  `TokenRange` so tooling can map back into trivia-preserving source
- validation entry points: `aivi_syntax::lex_module` establishes token/trivia invariants and
  `aivi_syntax::parse_module` establishes CST shape plus recoverable syntax diagnostics
- losslessness: comments, whitespace, and other trivia remain in the token buffer even when the
  structured tree does not lower them into dedicated CST nodes

### 4.2 HIR

HIR is the first module-owned arena IR.

Boundary contract:

- ownership: one `aivi_hir::Module` owns arenas for items, expressions, patterns, decorators,
  bindings, markup nodes, control nodes, and type nodes
- identity: opaque arena ids such as `ItemId`, `ExprId`, `PatternId`, `DecoratorId`,
  `MarkupNodeId`, and `ControlNodeId`
- source spans: every user-facing name, item header, expression, pattern, markup node, and control
  node carries the source span that diagnostics must report
- validation entry points: `aivi_hir::lower_module` / `lower_module_with_resolver`,
  `aivi_hir::validate_module`, and `aivi_hir::typecheck_module`

HIR responsibilities:

- names resolved
- imports resolved
- decorators attached
- markup nodes represented explicitly
- pipe clusters represented explicitly
- surface sugar preserved where useful for diagnostics
- source metadata and source-lifecycle/decode/fanout/recurrence elaboration reports made explicit
- body-less annotated `sig` declarations preserved as first-class input signals rather than erased

### 4.3 Typed core

Typed core is the first post-HIR layer that owns fully typed runtime-facing nodes rather than
resolved surface syntax.

Boundary contract:

- ownership: one `aivi_core::Module` owns typed arenas for items, expressions, pipes, stages,
  sources, and decode programs
- identity: opaque ids such as `ItemId`, `ExprId`, `PipeId`, `StageId`, `SourceId`,
  `DecodeProgramId`, and `DecodeStepId`
- source spans: expressions, patterns, stages, items, source nodes, and decode nodes preserve
  source spans; origin handles back into HIR stay attached where later layers need them
- validation entry points: `aivi_core::lower_module`, `aivi_core::lower_runtime_module`, and
  `aivi_core::validate_module`

Typed core responsibilities:

- all names resolved
- kinds checked
- class constraints attached
- `&|>` normalized into applicative spines
- pattern matching normalized
- record default elision elaborated
- markup control nodes typed
- signal dependency graph extracted
- blocked or not-yet-proven ordinary expression slices kept explicit rather than guessed into core

### 4.4 Closed typed lambda IR

The typed lambda layer keeps closure structure explicit without collapsing directly into backend
layout or ABI choices.

Boundary contract:

- ownership: one `aivi_lambda::Module` owns closure and capture arenas while embedding the
  validated typed-core module it wraps
- identity: explicit `ClosureId` and `CaptureId` plus carried-through core ids for items, pipes,
  stages, sources, and decode programs
- source spans: closure, item, pipe, and stage nodes preserve source spans from typed core / HIR
- validation entry points: `aivi_lambda::lower_module` and `aivi_lambda::validate_module`

Responsibilities:

- explicit closures
- explicit environments
- explicit runtime nodes for sources/tasks/signals where needed
- dictionary passing or monomorphization decisions applied
- no remaining surface sugar

### 4.5 Backend IR and codegen

Backend IR is the first layer that owns ABI/layout/runtime call contracts outright.

Boundary contract:

- ownership: one backend `Program` owns items, pipelines, kernels, layouts, sources, and decode
  plans
- identity: backend-owned ids such as `PipelineId`, `KernelId`, `KernelExprId`, `LayoutId`,
  `SourceId`, `DecodePlanId`, `DecodeStepId`, `EnvSlotId`, and `InlineSubjectId`, plus origin
  links back into earlier IRs
- source spans: item, pipeline, stage, source, and kernel origins preserve source spans; backend
  expressions keep source spans for diagnostics and debug dumps
- validation entry points: `aivi_backend::lower_module`, `aivi_backend::validate_program`, and
  `aivi_backend::compile_program`

Responsibilities:

- layout decisions
- concrete calling conventions
- Cranelift lowering
- AOT and JIT support

---

## 5. Top-level forms

Canonical top-level declarations:

```aivi
type Bool = True | False

class Eq A
    (==) : A -> A -> Bool

val answer = 42

fun add:Int #x:Int #y:Int =>
    x + y

sig counter = 0

use aivi.network (
    http
    socket
)
```

The core top-level forms are:

- `type`
- `class`
- `instance`
- `val`
- `fun`
- `sig`
- `use`
- `export`
- `provider`
- decorators via `@name`

A module may export at most one conventional process-entry binding named `main`.

When present, `main` remains the conventional standalone-process entry for future packaging and full
runtime/link integration. The current `aivi run` surface is preview-oriented and does **not**
privilege `main` over the static view-selection rules in §26.3. A top-level markup-valued `val`
named `view` is the preferred unqualified preview entry when no explicit `--view` is given.

Comment syntax in v1:

- `//` starts a line comment and runs to end of line
- `/* ... */` is a block comment (may span multiple lines)
- `/** ... **/` is a doc comment (may span multiple lines)
- all three forms are trivia in the lossless token stream; they do not create ordinary expression or
  item nodes in the CST
- the lexical distinction between `//`, `/* */`, and `/** **/` is stable; declaration attachment
  and doc extraction remain tooling-owned work above the syntax layer

### 5.1 Import rules

Name lookup is intentionally simple in v1:

- local names and a small explicit import set work
- no wildcard imports in v1
- no arbitrary value-level module qualification for imported module members in v1
- compiler-known type/domain/class namespaces may still expose qualified member lookup syntax such
  as `Duration.value`, `Text.join`, or `Foldable.reduce`
- built-in names keep priority where needed
- callable domain members and class members participate in ordinary term lookup when they are in
  scope; this is still term lookup, not projection syntax

#### Import aliases

`use module (member as localName)` is the disambiguation escape hatch when two imports would
otherwise provide the same local name:

```aivi
use aivi.network (http)
use my.client (fetch as clientFetch)
```

The original member name still drives compiler-known metadata. The alias changes only the local
binding name.

#### Name resolution for terms

The compiler prefers unqualified term use and resolves the right binding from local name plus
already-known context. When several candidates remain after contextual filtering, the compiler
reports an ambiguity diagnostic and requires explicit disambiguation through an import alias.

Qualified member syntax such as `Duration.value` or `Text.join` is therefore not a general
module-qualification escape hatch. It is a compiler-known lookup surface for the owning
 type/domain/class namespace only.

---

## 6. Type system

## 6.1 Kinds

AIVI includes a small explicit kind system.

Base kind:

- `Type`

Constructor kinds:

- `Type -> Type`
- `Type -> Type -> Type`
- right-associative arrow kinds

Examples:

- `Int : Type`
- `Text : Type`
- `Option : Type -> Type`
- `Signal : Type -> Type`
- `Result : Type -> Type -> Type`
- `Task : Type -> Type -> Type`

Partial application of named type constructors is supported.

Valid examples:

- `Option`
- `List`
- `Signal`
- `Result HttpError`
- `Task FsError`

Invalid examples:

- passing `Result` where a unary constructor is required
- passing `List Int` where a constructor is required

Full type-level lambdas are deferred.

## 6.2 Core primitive and standard types

Minimum practical v1 set:

- `Int`
- `Float`
- `Decimal`
- `BigInt`
- `Bool`
- `Text`
- `Unit`
- `Bytes`
- `List A`
- `Map K V`
- `Set A`
- `Option A`
- `Result E A`
- `Validation E A`
- `Signal A`
- `Task E A`

### 6.2.1 Numeric literal surface in v1

The implemented v1 literal surface is intentionally narrower than the full set of numeric types
listed above.

Accepted surface forms:

- unsuffixed integer literals are ASCII decimal digits only: `0`, `42`, `9000`
- built-in float literals are ASCII decimal digits, one `.`, and ASCII decimal digits:
  `0.5`, `3.14`
- built-in decimal literals are ASCII decimal digits with a trailing `d`, optionally with one
  fractional `.<digits>` part before the suffix: `19d`, `19.25d`
- built-in BigInt literals are ASCII decimal digits with a trailing `n`: `123n`
- a compact `digits + suffix` form is parsed as a domain literal suffix candidate only when the
  suffix is at least two ASCII letters and the token does not match one of the built-in non-`Int`
  literal forms: `250ms`, `10sec`, `3min`
- spacing is semantic: `250ms` is one suffixed literal candidate, while `250 ms` is ordinary
  application
- leading zeroes do not introduce octal or any other alternate base; `007` is decimal
- exact one-letter alphabetic compact suffixes are reserved for built-in numeric literal families
  and future core numeric extensions
- in the current surface, `d` and `n` are allocated to the built-in `Decimal` / `BigInt` literal
  families; user-defined and domain-defined suffixes must therefore use two or more letters

Not part of the v1 literal grammar:

- sign-prefixed numeric literals
- `_` separators inside numeric tokens
- built-in hex, binary, or octal integer forms
- exponent notation

A compact suffix form is only well-typed when exactly one current-module domain literal suffix
claims that suffix name and accepts the base integer family. Domain suffixes in v1 are
multi-character only. Otherwise the literal is rejected during later validation as an unresolved or
ambiguous suffix literal.

### 6.2.2 Executable numeric literal slice

The current executable backend/runtime slice intentionally stops short of a general numeric tower.

- `Int` literals execute as by-value `i64`.
- `Float` literals execute as finite IEEE-754 `f64` values and keep the backend's native by-value
  scalar ABI.
- `Decimal` literals execute as exact decimal runtime values, but backend layout marks them
  by-reference and Cranelift materializes them only as immutable literal cells with
  `mantissa:i128 (little-endian) + scale:u32 (little-endian)`.
- `BigInt` literals execute as exact arbitrary-precision integer runtime values, but backend layout
  marks them by-reference and Cranelift materializes them only as immutable literal cells with
  `sign:u8 + 7 bytes padding + byte_len:u64 (little-endian) + magnitude bytes (little-endian)`.
- `Decimal` and `BigInt` literal cells are introduction-only in the current Cranelift slice. This
  is an explicit layout/runtime boundary, not an implicit promise of full decimal/bignum arithmetic
  in backend codegen yet.
- Non-`Int` arithmetic and ordered comparison remain deferred in the executable backend slice even
  though the parser, HIR, and literal execution path recognize these builtin literal families.
- Diagnostics must preserve the user's raw numeric spelling for all literal families.

## 6.3 Closed types

Closed types mean:

- no `null` inhabitants unless represented explicitly in an ADT
- records are closed by default
- sums are closed by default
- missing or extra decoded fields are errors by default
- exhaustiveness checking is available for closed sums

## 6.4 Product types and data constructors

Constructor-headed product declarations are the default product form.

```aivi
type Vec2 = Vec2 Int Int
type Date = Date Year Month Day
```

### 6.4.1 Term-level constructor semantics

Every non-record ADT constructor is an ordinary curried value constructor.

```aivi
type Result E A = Err E | Ok A

val ok  = Ok
val one = Ok 1
```

Under-application is legal. Exact application constructs the value. Over-application is a type error.

This applies to both unary and multi-argument constructors.

### 6.4.2 Record construction

Records are built with record literals, not implicit curried record constructors.

```aivi
type User = { name: Text, age: Int }

val u:User = { name: "Ada", age: 36 }
```

### 6.4.3 Opaque and branded types

Opaque or branded types are recommended for domain-safe wrappers such as `Year`, `Month`, `Path`, `Url`, `Color`, and `Duration`. Public unary constructors are appropriate only when constructor application is intentionally part of the surface API.

## 6.5 Sum types

Canonical sum syntax:

```aivi
type Bool = True | False

type Option A =
  | None
  | Some A
```

Nested constructor patterns are allowed. Exhaustiveness is required for sum matches unless a wildcard is present.

## 6.6 Records, tuples, and lists

Value forms:

```aivi
(1, 2)
{ name: "Ada", age: 36 }
[1, 2, 3]
```

- tuples are positional products
- records are named products
- lists are homogeneous sequences

## 6.7 Maps and sets

Collection literal forms:

```aivi
Map { "x": 1, "y": 2 }
Set [1, 2, 4]
```

Rules:

- plain `{ ... }` is always a record
- plain `[ ... ]` is always a list
- duplicate record fields are a compile-time error
- duplicate map keys are a compile-time error
- duplicate set entries are allowed but may be warned and deduplicated

---

## 7. Core abstraction model

AIVI includes a small class/instance abstraction mechanism. In the current implementation slice,
core typeclasses are compiler-owned ambient prelude items injected into every checked module; local
declarations may shadow them. This keeps the surface explicit without inventing fake imports.

Constraint syntax is shared across class heads, class members, functions, and instance heads:

```aivi
class Functor F
    map : (A -> B) -> F A -> F B

class Functor F => Applicative F
    pure  : A -> F A
    apply : F (A -> B) -> F A -> F B

class (Eq A, Show A) => Example A
    render : A -> Text
```

The surface form `Constraint => ...` attaches a single constraint; `(C1, C2) => ...` attaches
multiple constraints.

### 7.1 Resolution rules

- instance resolution is coherent
- overlapping instances are not allowed in v1
- orphan instances are **fully disallowed** in v1
- instance search is compile-time only
- user-authored instance lookup is currently same-module only; imported user instances remain
  deferred
- unary `instance` blocks with indented member bindings are the implemented surface; instance
  contexts are deferred
- instance bodies are checked directly against the class-member arrow types with explicit local
  parameter bindings

### 7.1.1 Overloaded term lookup

Class members are overloaded term candidates. Ambient-prelude and same-module class members enter
term lookup, and evidence selection is driven by concrete argument/result types that the checker can
prove locally.

This does **not** imply open-ended global search or unrestricted higher-rank inference. The current
surface remains deliberately narrow:

- evidence must be concrete enough for checked HIR to choose a member
- imported polymorphic class-member execution remains deferred
- unresolved or multiply valid candidates are diagnosed explicitly rather than guessed

### 7.1.2 Lowering strategy

Checked HIR records the chosen class member, subject binding, and evidence source explicitly.
Typed core lowers the builtin runtime-supported class-member surface to intrinsic references for:

- `map`
- `pure`
- `apply`
- `reduce`
- `append`
- `empty`
- `compare`
- structural equality

Same-module instance members lower as hidden callable items per `(instance, member)`. Overloaded
references point to those hidden callables rather than asking later layers to rediscover instance
selection.

### 7.2 Core instances

Recommended v1 instances:

- `Option` implements `Functor`, `Applicative`, `Monad`
- `Result E` implements `Functor`, `Applicative`, `Monad`
- `List` implements `Functor`, `Applicative`, `Monad`
- `Task E` implements `Functor`, `Applicative`, `Monad`
- `Signal` implements `Functor`, `Applicative`
- `Validation E` implements `Functor`, `Applicative`
- `Eq` is compiler-provided for the structural cases in §7.3
- `Default` participates through the same evidence machinery used for other constraints

### 7.2.1 `Foldable.reduce`

`Foldable.reduce` is the current compiler-provided reduction surface for builtin collection/error
carriers:

- `List A` folds left-to-right in source order
- `Option A` folds zero or one payloads: `None` returns the seed unchanged, `Some x` applies the
  step once
- `Result E A` folds over the success payload only: `Err _` returns the seed unchanged, `Ok x`
  applies the step once
- `Validation E A` folds over the valid payload only: `Invalid _` returns the seed unchanged,
  `Valid x` applies the step once

This surface is intentionally narrow: it preserves the applicative meaning of `Validation` and
does not imply any `Foldable Task` or `Foldable Signal` instance in v1.

### 7.3 Equality

AIVI includes a first-order equality class:

```aivi
class Eq A
    (==) : A -> A -> Bool
```

`Eq` uses the ordinary class/instance resolution rules in §7.1. In the current implementation
slice, compiler-derived and builtin evidence covers the executable surface; user-authored `Eq`
instances beyond same-module explicit evidence remain deferred.

Compiler-derived `Eq` instances are required for:

- primitive scalars: `Int`, `Float`, `Decimal`, `BigInt`, `Bool`, `Text`, `Unit`
- tuples whose element types are `Eq`
- closed records whose field types are `Eq`
- closed sums whose constructor payload types are all `Eq`
- constructor-headed product declarations through the same closed-sum rule
- `List A` and `Option A` when `A` is `Eq`
- `Result E A` and `Validation E A` when both `E` and `A` are `Eq`
- domains whose underlying carrier supports `Eq`, while still preserving domain identity

Derived equality is structural and type-directed:

- tuple equality is position-by-position
- record equality is fieldwise over the declared closed field set
- sum equality compares constructor tags first, then constructor payloads
- list equality is length- and order-sensitive
- primitive scalar equality is same-type only; it is not coercive or approximate

`Eq` is not compiler-derived in v1 for `Bytes`, `Map`, `Set`, `Signal`, `Task`, function values,
GTK/foreign handles, or other runtime-managed boundary types whose equality semantics have not yet
been specified.

### 7.4 Non-instances

`Signal` is **not** a `Monad` in v1.

Rationale:

- monadic signals tend to imply dynamic dependency rewiring
- that complicates graph extraction, scheduling, teardown, and diagnostics
- AIVI wants a static, explicit, topologically scheduled signal graph

`Validation E` is **not** a `Monad` in v1 because the intended accumulation semantics are
applicative rather than dependent short-circuiting.

### 7.5 Laws

The standard semantic laws are normative for lawful instances:

- `Eq`: reflexivity, symmetry, transitivity
- `Functor`: identity, composition
- `Applicative`: identity, homomorphism, interchange, composition
- `Monad`: left identity, right identity, associativity

The compiler is not required to prove these laws.

---

## 8. Validation

`Validation E A` is a standard-library ADT for independent error accumulation.

```aivi
type Validation E A =
  | Invalid (NonEmptyList E)
  | Valid A
```

Unlike `Result E A`, the applicative instance for `Validation E` accumulates independent errors instead of short-circuiting on the first failure.

### 8.1 Applicative semantics

For `Validation E`, applicative combination behaves as follows:

- `pure x` yields `Valid x`
- applying `Valid f` to `Valid x` yields `Valid (f x)`
- applying `Invalid e` to `Valid _` yields `Invalid e`
- applying `Valid _` to `Invalid e` yields `Invalid e`
- applying `Invalid e1` to `Invalid e2` yields `Invalid (e1 ++ e2)`

Here `++` is concatenation of the underlying `NonEmptyList E`.

### 8.2 Intent

`Validation` is the canonical carrier for form validation under `&|>` because the inputs are independent and all failures should be reported together.

Example:

```aivi
sig validatedUser =
 &|> validateName nameText
 &|> validateEmail emailText
 &|> validateAge ageText
  |> UserDraft
```

If all validators succeed, the result is `Valid (UserDraft ...)`.
If one or more validators fail, all reported errors are accumulated into one `Invalid` value in source order.

`Validation E` is intentionally applicative-only in v1. Dependent validation that requires earlier successful values to choose later checks should use `Result`, `Task`, or explicit pattern matching instead.

---

## 9. Defaults and record omission

Defaulting is explicit and scoped. It does not make records open.

### 9.1 Default class

AIVI includes a small defaulting class:

```aivi
class Default A
    default : A
```

### 9.2 `aivi.defaults`

The module `aivi.defaults` exports instance bundles. The required first bundle is:

```aivi
use aivi.defaults (Option)
```

which brings the following idea into scope:

```aivi
Default (Option A)
default = None
```

### 9.3 Record literal elision

When an expected closed record type is known, omitted fields are filled only if a `Default` instance is in scope for every omitted field type.

Example:

```aivi
type User = {
    name: Text,
    nickname: Option Text,
    email: Option Text
}

use aivi.defaults (Option)

val user:User = { name: "Ada" }
```

This elaborates to:

```aivi
val user:User = {
    name: "Ada",
    nickname: None,
    email: None
}
```

### 9.4 Record shorthand

When an expected closed record type is known, a field whose label and in-scope value name coincide may be written in shorthand form.

Example:

```aivi
val snake = initialSnake
val food = initialFood
val status = Running
val score = 0

val game:Game = {
    snake,
    food,
    status,
    score
}
```

This elaborates to:

```aivi
val game:Game = {
    snake: snake,
    food: food,
    status: status,
    score: score
}
```

The same shorthand is allowed in record patterns.

```aivi
game
 ||> { snake, food, status, score } => score
```

This elaborates to:

```aivi
game
 ||> { snake: snake, food: food, status: status, score: score } => score
```

Shorthand is legal only when:

- the expected record type is known
- the field name exists on that closed record type
- a local binding of the same name is in scope for record construction
- the shorthand is unambiguous in patterns

Shorthand does not introduce open records, punning across different field names, or implicit defaults.

### 9.5 Restrictions

Omission is legal only when:

- the expected record type is known
- each omitted field has a `Default` instance in scope

This feature does **not**:

- open records
- change pattern matching semantics
- weaken strict source decoding
- add runtime fallback guessing

---

## 10. Expression model and control flow

AIVI is expression-first.

### 10.1 No `if` / `else`

AIVI does not use `if` / `else`. Branching uses pattern matching or predicate-gated flow.

### 10.2 No loops

The surface language has no imperative loop constructs. Repetition is expressed through:

- recursion
- collection combinators
- source/retry/interval flows
- controlled recurrent pipe forms

### 10.3 Ambient subject

Within a pipe, there is a current ambient subject.

- `_` means the entire current subject
- `.field` projects from the current subject
- `.field.subfield` chains projection
- `.field` is illegal where no ambient subject exists

---

## 11. Pipe algebra

Pipe algebra is one of AIVI's defining surface features.

## 11.1 Operators

Core v1 operators:

- ` |>` transform
- `?|>` gate
- `||>` case split
- `*|>` map / fan-out
- `&|>` applicative cluster stage
- `@|>` recurrent flow start
- `<|@` recurrence step
- ` | ` tap
- `<|*` fan-out join

Ordinary expression precedence, from tighter to looser binding:

1. function application
2. binary `+` and `-`
3. binary `>`, `<`, `==`, `!=`
4. `and`
5. `or`

Operators at the same binary precedence associate left-to-right.

The current surface subset also supports prefix `not`; it applies to its following ordinary
expression before binary reassociation.

Pipe operators are **not** part of that binary table. A pipe spine starts from one ordinary
expression head, then consumes pipe stages left-to-right. Each stage payload is parsed as an
ordinary expression using the table above until the next pipe operator boundary.

Reactivity does **not** come from pipe operators. Reactivity comes from `sig` and `@source`. Pipe operators are flow combinators inside those reactive or ordinary expressions.

### 11.2 `|>` transform

Transforms the current subject into a new subject.

```aivi
order |> .status
```

### 11.3 `?|>` gate

Allows the current subject through only if the predicate holds.

```aivi
users ?|> .active
```

The gate body is typed against the current ambient subject and must produce `Bool`.

Signal semantics:

- for `Signal A`, updates whose predicate is `True` are forwarded
- updates whose predicate is `False` are suppressed
- the result type remains `Signal A`
- no synthetic negative update is emitted

Ordinary-value semantics:

- for an ordinary subject `A`, `?|>` lowers to `Option A`
- success yields `Some subject`
- failure yields `None`

Example:

```aivi
user
 ?|> .active
 T|> .email
 F|> "inactive"
```

This is the canonical expression-level replacement for keeping or dropping a value without introducing `if` / `else`.

Restrictions:

- the predicate must be pure
- `?|>` is not a general branch operator; use `||>` when the two paths compute unrelated shapes
- `?|>` does not inspect prior history or future updates; it is pointwise over the current subject

### 11.4 `||>` case split

Performs pattern matching over the current subject.

```aivi
status
 ||> Paid    => "paid"
 ||> Pending => "pending"
```

### 11.4.1 `T|>` and `F|>` truthy / falsy branching

`T|>` and `F|>` are shorthand predicate-gated branch operators for carriers with canonical
positive and negative constructors.

They are surface sugar over `||>` and elaborate deterministically.

Boolean example:

```aivi
ready
 T|> start
 F|> wait
```

elaborates to:

```aivi
ready
 ||> True  => start
 ||> False => wait
```

`Option` example:

```aivi
maybeUser
 T|> greet _
 F|> showLogin
```

elaborates to:

```aivi
maybeUser
 ||> Some a => greet a
 ||> None   => showLogin
```

`Result` example:

```aivi
loaded
 T|> render _
 F|> showError _
```

elaborates to:

```aivi
loaded
 ||> Ok a  => render a
 ||> Err e => showError e
```

The canonical truthy / falsy constructor pairs in v1 are:

- `True` / `False`
- `Some _` / `None`
- `Ok _` / `Err _`
- `Valid _` / `Invalid _`

A single outer `Signal` lift is also part of the implemented surface. `Signal Bool`,
`Signal (Option A)`, `Signal (Result E A)`, and `Signal (Validation E A)` reuse the same carrier
plan pointwise: the runtime types the inner carrier, elaborates the branches, and then re-wraps the
result as `Signal`.

Rules:

- `T|>` and `F|>` may appear only as an adjacent pair within one pipe spine
- the subject type must have a known canonical truthy / falsy pair
- inside a `T|>` or `F|>` body, `_` is rebound to the matched payload when that constructor has
  exactly one payload
- zero-payload cases such as `True`, `False`, and `None` do not introduce a branch payload
- non-canonical inner carriers under `Signal` are rejected
- use `||>` when named binding, nested patterns, or more than two constructors are required

### 11.5 `*|>` map / fan-out

Maps over each element of a collection.

```aivi
users
 *|> .email
```

Each element becomes the ambient subject within the fan-out body.

Typing and lowering rules:

- for `List A`, `*|>` maps `A -> B` to produce `List B`
- for `Signal (List A)`, fan-out is lifted pointwise to produce `Signal (List B)`
- the body is typed as if it were a normal pipe body with the element as ambient subject
- the outer collection is not implicitly ambient inside the body; capture it by name if needed

`*|>` is pure mapping only. It does not implicitly flatten nested collections, sequence `Task`s, or merge nested `Signal`s.

### 11.5.1 `<|*` fan-out join

Joins the collection produced by the immediately preceding `*|>` with an explicit reducer.

```aivi
users
 *|> .email
 <|* Text.join ", "
```

`xs *|> f <|* g` elaborates to `g (map f xs)`.

For `Signal (List A)`, the same rule is lifted pointwise over signal updates.

Restrictions:

- `<|*` is legal only immediately after a `*|>` segment
- the join function is explicit; there is no implicit flattening or collection-specific default join

### 11.6 `|` tap

Observes the subject without changing it.

```aivi
value
 |> compute
 |  debug
 |> finish
```

The tap body is evaluated with the current subject as ambient subject. Its result is ignored. The outgoing subject is exactly the incoming subject.

Conceptually, `x | f` behaves like `let _ = f x in x`.

`|` is intended for tracing, metrics, and named observers. It is not a hidden mutation or control-flow channel.

### 11.7 `@|>` and `<|@`

These mark explicit recurrent flows used for retry, polling, and stream-style pipelines.

`@|>` enters a recurrent region. Each subsequent `<|@` stage contributes to the per-iteration step function over the current loop state.

Conceptually, a recurrent spine denotes a scheduler-owned loop node rather than direct self-recursion. The current iteration value is the ambient subject within the recurrent region.

Normative v1 rules:

- recurrent pipes are legal only where the compiler can lower them to a built-in runtime node for `Task`, `Signal`, or `@source` helpers
- recurrence wakeups must be explicit: timer, backoff, source event, or provider-defined trigger
- each iteration is scheduled and stack-safe; recurrent pipes must not lower to unbounded direct recursion
- cancellation or owner teardown disposes the pending recurrence immediately
- if the compiler cannot determine a valid runtime lowering target, the recurrent pipe is rejected

---

## 12. Exact applicative surface semantics for `&|>`

This section is normative.

## 12.1 Intent

`&|>` is the surface operator for **applicative clustering**: combining independent effectful/reactive values under a shared `Applicative` and then applying a pure constructor or function.

It is intended for:

- form validation
- combining independent signals
- assembling values from independent `Option`, `Result`, `Validation`, or `Task` computations

It is **not**:

- monadic sequencing
- short-circuit imperative flow
- ad-hoc tuple syntax

## 12.2 Surface forms

A cluster may start either from an ordinary expression or from a leading cluster stage.

### Expression-headed cluster

```aivi
 &|> validateName nameText
 &|> validateEmail emailText
 &|> validateAge ageText
  |> UserDraft
```

### Leading cluster form

A leading `&|>` is legal at the start of a pipe spine or multiline body.

```aivi
sig validatedUser =
 &|> validateName nameText
 &|> validateEmail emailText
 &|> validateAge ageText
  |> UserDraft
```

This form is preferred when scanning a validation spine because every independent input aligns at the operator.

## 12.3 Grammar shape

Conceptually:

```text
ApplicativeCluster ::=
    ClusterHead ClusterTail+ Finalizer?
  | LeadingClusterHead ClusterTail+ Finalizer?

ClusterHead        ::= Expr
LeadingClusterHead ::= "&|>" Expr
ClusterTail        ::= "&|>" Expr
Finalizer          ::= " |>" Expr
```

`ApplicativeCluster` is a surface form only. It does not survive into backend-facing IR.

## 12.4 Typing rule

All cluster members must have the same outer applicative constructor `F`.

Examples of legal clusters:

- `Validation FormError A`
- `Signal A`
- `Option A`
- `Result HttpError A`
- `Task FsError A`

All cluster members in one cluster must be of shape `F Ai` for the same `F`.

## 12.5 Desugaring

A finished cluster:

```aivi
 &|> a
 &|> b
 &|> c
  |> f
```

desugars to:

```aivi
pure f
    |> apply a
    |> apply b
    |> apply c
```

which is equivalent to:

```aivi
apply (apply (apply (pure f) a) b) c
```

The leading form:

```aivi
&|> a
&|> b
&|> c
 |> f
```

desugars the same way.

## 12.6 End-of-cluster default

If a cluster reaches pipe end without an explicit finalizer, it finalizes to a tuple constructor of matching arity.

```aivi
&|> a
&|> b
```

desugars to:

```aivi
pure Tuple2
    |> apply a
    |> apply b
```

Implementations may represent these tuple constructors internally; the surface semantics are tuple formation.

## 12.7 Restrictions

Inside an unfinished applicative cluster:

- ambient-subject projections such as `.field` are illegal unless they occur inside a nested expression whose own subject is explicit
- `?|>` and `||>` are illegal until the cluster is finalized
- the finalizer must be a pure function or constructor from the user's perspective

These restrictions keep the operator law-abiding and make elaboration deterministic.

## 12.8 Examples

### Validation

```aivi
sig validatedUser =
 &|> validateName nameText
 &|> validateEmail emailText
 &|> validateAge ageText
  |> UserDraft
```

### Signals

```aivi
sig fullName =
 &|> firstName
 &|> lastName
  |> joinName
```

### Result

```aivi
val loaded =
 &|> readConfig path
 &|> readSchema schemaPath
  |> buildRuntimeConfig
```

## 12.9 `Signal` interaction

For `Signal`, `&|>` builds a derived signal whose dependencies are the union of the member dependencies. The result observes the latest stable upstream values per scheduler tick.

That is applicative combination, not signal monadic binding.

---

## 13. Signals and scheduler semantics

`sig` introduces a reactive binding.

```aivi
sig x = 3
sig y = x + 5
```

A signal referenced inside a `sig` is read as its current committed value during evaluation of that
`sig`. The enclosing `sig` becomes dependent on every **locally provable** signal referenced in its
definition.

### 13.1 Rules

- `sig` is the reactive boundary
- `val` must not depend on signals
- pure helper functions used inside `sig` stay pure
- signal dependency extraction happens after elaboration
- ordinary derived-signal dependency graphs are static after elaboration
- all signals carry explicit local dependency lists for scheduling and diagnostics
- source-backed signals record local signal dependencies only; imported references are not assumed
  to be publishable signals unless the compiler has explicit proof

### 13.2 Input signals

An annotated body-less `sig` declaration is a first-class input signal — an externally publishable
entry point for reactive inputs such as GTK events, tests, and runtime-owned completions.

```aivi
sig clicked : Signal Unit
sig query   : Signal Text
```

These are not errors. They define runtime-owned slots that external code may publish into. Their
type annotation is mandatory. They participate in the signal dependency graph exactly like derived
signals, but their publication port is owned by the runtime rather than by user code.

Input signals are the canonical mechanism for routing GTK event payloads into the language-level
reactive graph. They are also the publication target used internally for task completions and other
runtime-owned boundaries.

### 13.3 Applicative meaning of `Signal`

`pure x` creates a constant signal.

`apply : Signal (A -> B) -> Signal A -> Signal B` creates a derived signal node with:

- dependency set equal to the union of the input dependencies
- latest-value semantics
- transactional visibility per scheduler tick
- glitch-free propagation

`Signal` remains applicative rather than monadic. Dynamic rewiring must be expressed through
explicit runtime/source nodes rather than through `bind`.

### 13.4 Scheduler guarantees

The runtime scheduler must provide:

- topological propagation order
- committed-snapshot evaluation per tick
- no mixed-time intermediate observations
- deterministic behavior for a fixed input event order
- generation-stamped publication so stale source/task results are rejected before propagation
- recursive owner disposal so torn-down subtrees deactivate their dependent runtime-owned nodes

The scheduler is driven from an owned GLib main context. Worker threads may publish results and
request wakeups, but they do not mutate scheduler-owned state directly.

### 13.5 No `Monad Signal`

AIVI v1 does not expose `bind` for `Signal`.

Any feature that would imply dynamic dependency rewiring must be expressed through explicit
source/runtime nodes rather than a general `Monad Signal`.

---

## 14. Sources and decoding

External inputs enter through `@source` on `sig`.

```aivi
@source http.get "/users"
sig users : Signal (Result HttpError (List User))
```

Source arguments and options are ordinary typed expressions. They may use interpolation and may
depend on signals whose dependency sets are statically known.

Example:

```aivi
@source http.get "{baseUrl}/users" with {
    headers: authHeaders,
    decode: Strict
}
sig users : Signal (Result HttpError (List User))
```

Reactive values in source strings, positional arguments, and options are real dependencies. When
those committed values change, the runtime rebuilds or retriggers the source according to the
provider contract while keeping the static graph shape fixed.

### 14.1 Source contract

A source is a runtime-owned producer that publishes typed values into the scheduler.

Sources may represent:

- HTTP
- file watching
- file reads
- sockets
- D-Bus
- timers
- process events
- mailboxes/channels
- GTK/window events

The HIR surface preserves, for every `@source` site:

- provider identity: missing / builtin / custom / invalid-shape
- positional arguments as runtime expressions
- options as runtime expressions
- lifecycle metadata
- decode program selection
- stable source instance identity

### 14.1.1 Recurrence decorators on non-source declarations

Plain repeating `sig` and `val` bodies prove their wakeup only through explicit `@recur.timer` or
`@recur.backoff` decorators. Each takes exactly one positional witness expression.

```aivi
@recur.timer 1000ms
sig polled : Signal Status

@recur.backoff initialDelay
sig retried : Signal (Result FetchError Data)
```

Rules:

- `@recur.timer expr` and `@recur.backoff expr` are the only recurrence decorators for non-`@source`
  declarations
- neither accepts `with { ... }` options or duplicates
- they are not allowed on `@source` signals; source wakeups come from the source contract
- a recurrent pipe is legal only where the compiler can prove a built-in runtime lowering target
- recurrence lowering produces an explicit scheduler-node handoff; it is not collapsed into opaque
  self-recursion

### 14.1.2 Source declaration shape

The general surface form is:

```aivi
@source provider.variant arg1 arg2 with {
    option1: value1,
    option2: value2
}
sig name : Signal T
```

The `with { ... }` option record is optional.

Minimal form:

```aivi
@source timer.every 120
sig tick : Signal Unit
```

Optioned form:

```aivi
@source http.get "/users" with {
    decode: Strict,
    retry: Retry.times 3,
    timeout: 5sec
}
sig users : Signal (Result HttpError (List User))
```

Rules:

- the provider and variant are resolved statically
- positional arguments are provider-defined and typed
- options are a closed record whose legal fields come from a central provider option catalog
- unknown options are a compile-time error
- duplicate options are a compile-time error
- value checking is intentionally staged: the compiler validates supported local closed shapes now
  and records explicit blockers for unsupported or unproven forms rather than guessing
- argument and option expressions may be ordinary values or signal-derived expressions with
  statically known dependencies
- reactive changes are split into three lifecycle classes: reconfiguration inputs,
  trigger/refresh inputs, and `activeWhen` gating inputs
- if a reactive reconfiguration input changes, the old runtime instance is superseded and a new one
  is created with a fresh generation
- imported option bindings are checked only when the import catalog provides an explicit closed
  value surface; otherwise the option remains deliberately unproven

Reactive source configuration does not make sources dynamic in the type-theoretic sense. The
provider kind and dependency graph remain statically known; only runtime configuration values
change.

### 14.1.3 Recommended v1 source variants

The following provider variants are recommended for v1.

#### HTTP

```aivi
@source http.get "/users"
sig users : Signal (Result HttpError (List User))

@source http.post "/login" with {
    body: creds,
    headers: authHeaders,
    decode: Strict,
    timeout: 5sec
}
sig login : Signal (Result HttpError Session)
```

Recommended HTTP options:

- `headers : Map Text Text`
- `query : Map Text Text`
- `body : A`
- `decode : DecodeMode`
- `timeout : Duration`
- `retry : Retry`
- `refreshOn : Signal B`
- `refreshEvery : Duration`
- `activeWhen : Signal Bool`

HTTP source semantics:

- refresh is explicit only: reactive config changes, `refreshOn`, `refreshEvery`, retries, or
  provider-defined intrinsic wakeups
- there are no lifecycle-event refreshes hidden behind GTK visibility or mount/unmount
- `refreshOn` reissues the request whenever the trigger signal updates
- `refreshEvery` creates scheduler-owned polling using the latest stable source configuration
- `activeWhen` gates startup and refresh; when it becomes `False`, polling is suspended and the
  current generation becomes inactive
- when reactive URL, query, header, or body inputs change, the runtime creates a replacement
  request generation using the latest committed values
- newer request generations supersede older ones; stale completions from superseded generations are
  dropped before publication
- built-in HTTP providers request best-effort cancellation of superseded or suspended requests

#### Timer

```aivi
@source timer.every 120
sig tick : Signal Unit

@source timer.after 1000
sig ready : Signal Unit
```

Recommended timer options:

- `immediate : Bool`
- `jitterMs : Int`
- `coalesce : Bool`
- `activeWhen : Signal Bool`

Bare integer timer arguments mean milliseconds. Common suffixed durations such as `250ms` are also
accepted when the duration domain surface is in place.

#### File watching and reading

```aivi
@source fs.watch "/tmp/demo.txt" with {
    events: [Created, Changed, Deleted]
}
sig fileEvents : Signal FsEvent

@source fs.read "/tmp/demo.txt" with {
    decode: Strict,
    reloadOn: fileEvents
}
sig fileText : Signal (Result FsError Text)
```

`fs.watch` publishes file-system change notifications only. It does **not** implicitly read file
contents. `fs.read` performs snapshot loading and decode. This split is normative.

Recommended file-watch options:

- `events : List FsWatchEvent`
- `recursive : Bool`

Recommended file-read options:

- `decode : DecodeMode`
- `reloadOn : Signal A`
- `debounce : Duration`
- `readOnStart : Bool`
- `activeWhen : Signal Bool`

Built-in file sources request best-effort cancellation when they are superseded, suspended, or
 torn down.

#### Socket / mailbox

```aivi
@source socket.connect "tcp://localhost:8080" with {
    decode: Strict
}
sig inbox : Signal (Result SocketError Message)

@source mailbox.subscribe "jobs"
sig jobs : Signal Text
```

Socket and mailbox semantics in the current runtime slice are intentionally transport-specific:

- `socket.connect` is a raw `tcp://` line-stream provider rather than a general WebSocket surface
- `mailbox.subscribe` is a process-local text bus
- unsupported options raise explicit runtime errors at provider registration rather than being
  silently ignored

#### Process events

```aivi
@source process.spawn "rg" ["TODO", "."]
sig grepEvents : Signal ProcessEvent
```

Recommended process options:

- `cwd : Path`
- `env : Map Text Text`
- `stdout : StreamMode`
- `stderr : StreamMode`
- `restartOn : Signal A`

#### GTK / window events

```aivi
@source window.keyDown with {
    repeat: False
}
sig keyDown : Signal Key
```

Recommended window-event options:

- `capture : Bool`
- `repeat : Bool`
- `focusOnly : Bool`

The current GTK host lowers `window.keyDown` through the focused window's key controller. This is a
provider-owned host boundary, not a generic DOM-like event model.

#### D-Bus

```aivi
@source dbus.ownName "org.example.Mail"
sig busName : Signal BusNameState

@source dbus.signal "org.example.Mail" "/org/example/Mail" "NewMessage"
sig busEvents : Signal MailBusEvent

@source dbus.method "org.example.Mail" "/org/example/Mail" "ShowWindow"
sig showWindow : Signal Unit
```

The user-facing D-Bus surface in this RFC slice is:

- `dbus.ownName` as a source tracking `Owned`, `Queued`, or `Lost`
- `dbus.signal` as a source subscribing to inbound remote signals
- `dbus.method` as a source for fire-and-forget inbound method dispatch where the Unit reply is
  placed on the D-Bus wire before the AIVI signal is published
- non-Unit reply-producing methods are deferred

### 14.1.4 Decode and delivery modes

Recommended supporting enums:

```aivi
type DecodeMode =
  | Strict
  | Permissive

type StreamMode =
  | Ignore
  | Lines
  | Bytes
```

Semantics:

- `Strict` rejects unknown or missing required fields according to closed-type decoding rules
- `Permissive` may ignore extra fields but still requires required fields unless the built-in decode
  surface says otherwise
- decode happens before scheduler publication
- delivery into the scheduler remains typed and transactional

### 14.2 Decoding

AIVI includes compiler-generated structural decoding by default.

Default decoding rules:

- closed records reject missing required fields
- extra fields are rejected in strict mode by default
- sum decoding is explicit
- decoder overrides are limited to the built-in decode surface; general custom decode hooks remain
  deferred
- domain-backed fields decode through the domain's explicit parser or constructor surface; they do
  not silently accept the raw carrier unless that surface says so

The runtime decode wire shape is explicit:

- payload bytes are first interpreted as UTF-8 text for providers that promise text transport
- plain `Text` targets accept the raw text unchanged
- structural targets decode from JSON
- closed sums, `Option`, `Result`, and `Validation` use a canonical JSON object shape
  `{ tag, payload }`
- unsupported scalar target families in the current provider startup slice (`Bytes`, `Float`,
  `Decimal`, `BigInt`, and domain-surface direct transport) fail explicitly at provider
  registration rather than later during silent coercion

Domain decode uses the following resolution order:

1. a domain-owned `parse` method when it has the shape `Carrier -> Result E Domain`
2. otherwise, a unique domain-owned `Carrier -> Domain` or `Carrier -> Result E Domain`
3. otherwise, decode is rejected as ambiguous or unsupported

Operator methods, literal methods, and multiply matching domain conversions are not decode
surfaces.

Record default elision for user-written literals does **not** weaken source decoding.
Decode failures flow through the source's typed error channel. They do not escape as untyped runtime
exceptions.

Regex literals are validated earlier, in HIR validation, rather than being delegated to source
providers.

### 14.3 Cancellation and lifecycle

Source subscriptions carry explicit runtime cancellation and disposal semantics. Every `@source`
site owns one stable runtime instance identity.

Lifecycle rules:

- lifecycle metadata distinguishes reactive reconfiguration, trigger, and `activeWhen` inputs
- reconfiguration caused by reactive source arguments or options replaces the superseded runtime
  resource transactionally from committed scheduler values
- stale work from a superseded, disposed, or inactive source generation is dropped and must never
  publish into the live graph
- `activeWhen` suspends delivery without changing the static graph shape
- request-like built-ins such as HTTP and `fs.read` request best-effort in-flight cancellation when
  they are replaced, suspended, or disposed
- built-in `SourceRuntimeSpec` values are validated against provider contracts at registration
- custom providers inherit the generic replacement and stale-publication rules, but built-in option
  names have semantics only when the provider contract declares them

### 14.4 Custom provider declarations

Custom source providers are declared at the top level with a `provider` keyword:

```aivi
provider my.data.source
    wakeup: providerTrigger
    argument url: Url
    option timeout: Duration
    option retries: Int
```

Implemented declaration rules:

- the provider name is a qualified top-level name
- `wakeup:` may currently be `timer`, `backoff`, `sourceEvent`, or `providerTrigger`
- unknown declaration fields are immediate diagnostics
- argument and option declarations are restricted to primitive types, same-module types,
  `List`, and `Signal` compositions over those closed shapes
- richer schemas are rejected at declaration time rather than being carried as vague metadata
- reactive source inputs always count as `sourceEvent` wakeups for any provider; non-reactive custom
  wakeups must be declared explicitly by the provider contract

---

## 15. Effects and `Task`

## 15.1 Purity boundary

Ordinary `val` and `fun` definitions are pure.

Effects enter through:

- `Task`
- `sig` / `@source`
- GTK event boundaries
- runtime-owned scheduling and source integration

## 15.2 `Task E A`

`Task E A` is the only user-visible one-shot effect carrier.

`Task`:

- describes a one-shot effectful computation
- may fail with `E`
- may succeed with `A`
- is schedulable by the runtime
- is lawful as `Functor`, `Applicative`, and `Monad`

Runtime execution uses linked task bindings plus scheduler-owned hidden completion inputs. A direct
top-level task value lowers to a `TaskRuntimeSpec`; a worker thread evaluates the linked backend
item body and publishes its result through a typed completion port back into the scheduler.

Recurrent `@|> ... <|@` tasks are outside the current executable slice and remain explicit runtime
blockers rather than being guessed into a looping task engine.

## 15.3 Event handler routing

The implemented GTK event surface is intentionally narrower than a future general callback
language.

In v1 live GTK routing:

- markup `on*={handler}` attributes are routing declarations, not arbitrary callback bodies
- `handler` must resolve to a directly publishable input signal declared as a body-less annotated
  `sig name : Signal T`
- the concrete GTK host must recognize the exact widget/event pair before the attribute is treated
  as live event routing
- the routed input signal payload type must match the concrete GTK event payload type
- handler resolution is performed once up front; GTK event payloads are then published directly into
  that input signal
- discrete GTK events publish one payload into the scheduler input signal and force their own
  runtime tick

Broader normalization of arbitrary handler expressions into runtime-owned actions remains future
work.

## 15.4 Inter-thread communication

The runtime uses explicit publication ports at worker boundaries. Workers receive read-only
cancellation observers and may publish source/task results back to the scheduler queue, but they do
not mutate GTK state or committed signal storage directly.

Library-level message-passing primitives may exist as runtime/library types:

```aivi
type Sender A
type Receiver A
type Mailbox A
```

with sending expressed through `Task` and receiving expressed through `@source` integration.

---

## 16. Runtime architecture

## 16.1 Memory management

The target runtime is a mostly-moving generational collector with incremental scheduling plus
narrow stable-handle support at foreign boundaries.

Language-visible guarantees:

- ordinary values may move
- stable addresses are not guaranteed
- GTK/GObject/FFI interactions use stable handles, pinned wrappers, or copied values
- values that cross GTK, worker, source-provider, or other foreign seams use explicit detached
  boundary wrappers or ports; boundary detachment is never implicit

Initial GC rollout scope is intentionally narrow: only scheduler-committed runtime snapshots are in
the moving-GC root set. Pending evaluator/source/task results remain ordinary Rust-owned values
until commit.

## 16.2 Threads

Recommended runtime shape:

- one GTK UI island on an owned GLib main context
- worker threads for I/O, decoding, task execution, and heavy fragment evaluation
- immutable message passing from workers to scheduler-owned queues
- no direct GTK mutation from workers

The GLib driver reentry rule is explicit: scheduler/evaluator ownership sits behind one guarded
critical section, and same-thread reentry is a runtime invariant violation rather than a best-effort
fallback.

## 16.3 Scheduler

The scheduler owns:

- signal propagation
- source event ingestion
- task completion publication
- cancellation/disposal
- tick boundaries
- committed runtime snapshots

The scheduler must be designed so that it cannot:

- block the GTK main loop during heavy work
- deadlock on normal cross-thread publication
- recurse unboundedly during propagation
- leak torn-down subscriptions
- accept stale publications from superseded generations

Committed scheduler state is the source of truth for runtime evaluation. Worker-computed results are
admitted only at tick-safe boundaries.

---

## 17. GTK / libadwaita embedding

AIVI's primary UI target is GTK4/libadwaita on Linux.

The pure language core remains pure. UI effects cross a controlled boundary through a typed GTK
bridge.

## 17.1 View model

AIVI uses typed markup-like view syntax and lowers it to a stable widget/binding graph.

It does **not** use a virtual DOM.

### 17.1.1 Direct lowering rules

The implementation path is explicit:

- HIR markup lowers to a typed `WidgetPlan` with stable identities, child operations, setter
  bindings, event hookups, and control branches
- `WidgetPlan` lowers to a `WidgetRuntimeAssembly` with concrete runtime handles and child-group
  structure
- the GTK executor consumes that runtime assembly through a bridge graph and applies direct GTK
  mutations

Each markup node therefore compiles to:

- widget/control-node kind
- static property initializers
- dynamic property bindings
- signal/event handlers
- child-slot instructions
- teardown logic

Ordinary widget nodes are created once per node identity. Dynamic props update through direct setter
calls. There is no generic diff engine over a virtual tree.

Live `aivi run` updates are planned off the GTK thread: the runtime snapshots committed globals on
the main thread, evaluates the selected view fragments on a worker, produces an immutable hydration
plan, and applies GTK mutations back on the main thread via `idle_add`.

## 17.2 Property and event binding

Example:

```aivi
<Label text={statusLabel order} visible={isVisible} />
```

If an expression is reactive, the compiler extracts a derived signal and the runtime:

- computes the initial value
- subscribes once
- calls the concrete GTK setter on change

Interpolated markup text remains genuinely dynamic. The GTK host routes interpolated text-valued
attributes through runtime setter bindings rather than freezing them into static property values.

### 17.2.1 Event hookups

Expression-valued markup attributes lower as live GTK event routes only when the widget schema
catalog declares that exact widget/event pair.

```aivi
sig clicked : Signal Unit

<Button label="Click me" onClick={clicked} />
```

Event hookup rules:

- the handler expression must name a directly publishable input signal
- only direct input signals are legal in the current live GTK surface; arbitrary callback
  expressions are future work
- the input signal's payload type must match the GTK event's concrete payload type
- unsupported event names on a given widget type remain ordinary attributes and are rejected by
  run-surface validation rather than silently treated as live events
- GTK discrete events force their own runtime ticks; rapid repeated events are processed as separate
  transactions and not collapsed within one generation

Attributes that start with `on` are treated as event-hook candidates only through this schema-backed
rule. The host does not guess event semantics from spelling alone.

### 17.2.2 Executable widget schema metadata

The live GTK host is driven by one compiled widget schema catalog shared by lowering, `aivi run`
validation, and concrete GTK hookup.

Each widget schema entry defines:

- the current markup lookup key
- property descriptors: exact property name, semantic value shape, and GTK setter route
- event descriptors: exact event name, GTK signal route, and payload shape
- child-group descriptors: group name, container policy, and child-count bounds
- whether the widget is window-like for root validation/presentation

In the current markup surface, unlabeled child content may populate only the schema's single
default child group. Widgets that need multiple named child groups remain deferred rather than being
filled by hidden host heuristics.

Current executable catalog:

- `Window` — properties `title`, `visible`, `sensitive`, `hexpand`, `vexpand`; no markup events;
  child group `content` accepting at most one child; treated as a window root
- `Box` — properties `orientation`, `spacing`, `visible`, `sensitive`, `hexpand`, `vexpand`; no
  markup events; child group `children` with append-only sequence semantics
- `ScrolledWindow` — properties `visible`, `sensitive`, `hexpand`, `vexpand`; no markup events;
  child group `content` accepting at most one child
- `Label` — properties `text`, `label`, `visible`, `sensitive`, `hexpand`, `vexpand`; no markup
  events; no child groups
- `Button` — properties `label`, `visible`, `sensitive`, `hexpand`, `vexpand`; event `onClick`
  publishing `Unit`; no child groups
- `Entry` — properties `text`, `placeholderText`, `editable`, `visible`, `sensitive`, `hexpand`,
  `vexpand`; event `onActivate` publishing `Unit`; no child groups
- `Switch` — properties `active`, `visible`, `sensitive`, `hexpand`, `vexpand`; no markup
  events; no child groups

Widgets outside this catalog are not part of the current live GTK surface.

### 17.2.3 Host lifecycle attributes

`trackVisible={sig}` is a host-backed lifecycle attribute that routes GTK `map` / `unmap` into a
user-declared `Signal Bool` input signal.

Rules:

- the bound signal must be a body-less annotated `Signal Bool` input signal
- the host publishes `False` immediately at registration, `True` on first `map`, and then `True` /
  `False` on later `map` / `unmap` transitions
- `map` / `unmap` is used rather than `show` / `hide` because a widget may be shown while still not
  mapped through an unshown parent
- this is the canonical way to drive `@source activeWhen` from visibility state

`hideOnClose={True}` on `ApplicationWindow` intercepts the delete event and calls `window.hide()`
instead of destroying the window. This keeps the process alive and allows later restoration through
normal presentation or an external activation path such as D-Bus.

## 17.3 Control nodes

Control nodes are part of the view language and lower directly.

### 17.3.1 `<show>`

```aivi
<show when={isVisible}>
    <Label text="Ready" />
</show>
```

Semantics:

- `when` must be `Bool`
- when false, the subtree is absent
- when true, the subtree is present

Optional flag:

```aivi
<show when={isVisible} keepMounted={True}>
    ...
</show>
```

- `keepMounted = False` is the default
- if `False`, hide means full subtree teardown per §17.4
- if `True`, the subtree mounts once and hide/show becomes a visibility transition rather than an
  unmount/remount cycle
- while hidden under `keepMounted = True`, property bindings, signal subscriptions, source
  subscriptions, and event hookups remain installed
- concrete input delivery while hidden follows the host toolkit; for the current GTK host,
  invisible widgets do not receive pointer or keyboard events even though their handlers remain
  connected

### 17.3.2 `<each>`

```aivi
<each of={items} as={item} key={item.id}>
    <Row item={item} />
</each>
```

Semantics:

- `of` must yield `List A`
- `as` binds the element within the body
- the body must produce valid child content for the parent slot
- `key` is required in the current RFC slice

Runtime behavior:

- child identity is maintained by key
- control-node planning is keyed by runtime handles, including the `<each>` key input
- updates compute localized child edits rather than whole-tree replacement
- existing child subtrees are reused by key where possible
- actual GTK child insertion/removal/reordering happens directly

This is localized child management, not virtual DOM diffing.

#### `<empty>`

`<each>` may optionally contain an `<empty>` branch rendered only when the list is empty.

```aivi
<each of={items} as={item} key={item.id}>
    <Row item={item} />
    <empty>
        <Label text="No items" />
    </empty>
</each>
```

### 17.3.3 `<match>`

Because the language has no `if` / `else`, markup supports direct pattern-based rendering.

```aivi
<match on={status}>
    <case pattern={Paid}>
        <Label text="Paid" />
    </case>
    <case pattern={Pending}>
        <Label text="Pending" />
    </case>
</match>
```

Rules:

- `on` is any expression
- cases use ordinary AIVI patterns
- exhaustiveness follows ordinary match rules where the scrutinee type is locally provable
- lowering selects and deselects concrete subtrees directly

### 17.3.4 `<fragment>`

```aivi
<fragment>
    <Label text="A" />
    <Label text="B" />
</fragment>
```

Groups children without creating a wrapper widget.

### 17.3.5 `<with>`

Useful local naming is allowed in markup through a non-reactive binding node:

```aivi
<with value={formatUser user} as={label}>
    <Label text={label} />
</with>
```

`<with>` introduces a pure local binding for the subtree. It does not create an independent signal
node.

## 17.4 Teardown and lifecycle

Tearing down a subtree must:

- disconnect event handlers
- dispose source subscriptions owned by that subtree
- release widget handles
- preserve correctness under repeated show/hide and keyed list churn
- recursively deactivate owned runtime nodes so stale publications are rejected after teardown

GTK correctness is part of the language runtime contract, not a best-effort library concern.

---

## 18. Pattern matching and predicates

Pattern matching is the main branching form in both ordinary expressions and markup control nodes.

### 18.1 Rules

- sum matches must be exhaustive unless `_` is present
- boolean matches must cover `True` and `False` unless `_` is present
- record patterns may be field-subset patterns
- nested constructor patterns are allowed

### 18.2 Predicates

Predicates may use:

- ambient projections such as `.age > 18`
- `_` for the current subject
- `and`, `or`, `not`
- `==` / `!=` when an `Eq` instance is available for the operand type

Examples:

```aivi
users |> filter (.active and .age > 18)
xs    |> takeWhile (_ < 10)
```

`x == y` desugars to `(==) x y`. `x != y` desugars to `not (x == y)` and does not introduce a separate class member.

---

## 19. Strings and regex

### 19.1 Text

String concatenation is not a core language feature. Text composition uses interpolation.

```aivi
"{name} ({status})"
```

### 19.2 Regex

Regex is a first-class compiled type with literal syntax such as:

```aivi
rx"\d{4}-\d{2}-\d{2}"
```

Invalid regex literals are compile-time errors.

Validation happens in HIR validation rather than in lexing. The compiler uses the Rust
`regex-syntax` acceptance surface to decide whether a regex literal is valid. This keeps the token
stream lossless while still making malformed regexes early, typed diagnostics.

---

## 20. Domains

Domains are nominal value spaces defined over an existing carrier type.

They are used when a value should:

- have the runtime representation of some existing type
- remain distinct at the type level
- optionally support domain-specific literal suffixes
- optionally expose domain-specific operators and smart constructors
- reject accidental mixing with the raw carrier or with other domains over the same carrier

Typical examples include:

- `Duration over Int`
- `Url over Text`
- `Path over Text`
- `Color over Int`
- `NonEmpty A over List A`

A domain is not a type alias. A domain is not subtyping. A domain does not imply implicit casts.

### 20.1 Declaration form

Canonical shape:

```aivi
domain Duration over Int
    literal ms  : Int -> Duration
    millis      : Int -> Duration
    parse       : Int -> Result DurationError Duration
    value       : Duration -> Int
```

Domains are first-class language declarations in v1.

### 20.2 Core meaning

A domain introduces a nominal type over a carrier type while preserving explicit construction and
elimination.

The domain, not the carrier, owns:

- literal suffixes
- smart construction
- explicit unwrapping
- domain-local operators
- optional decode/parse surfaces

### 20.3 Relation to opaque and branded types

Use `domain` when the nominal wrapper is expected to carry domain-owned literal, parsing, decode,
or operator surfaces. Use `type` when an ordinary ADT or record is enough.

### 20.4 Construction and elimination

A domain may be introduced only through domain-owned constructors or smart constructors.

Recommended surface shape:

```aivi
domain Url over Text
    parse : Text -> Result UrlError Url
    value : Url -> Text
```

```aivi
domain Duration over Int
    millis     : Int -> Duration
    trySeconds : Int -> Result DurationError Duration
    value      : Duration -> Int
```

Construction is explicit. Unwrapping is explicit. Unsafe construction should remain internal or be
spelled as such.

Callable domain members enter ordinary term lookup when they are in scope. There is no projection
syntax for domains in v1; the compiler may still expose compiler-known qualified lookup surfaces
such as `Duration.value` for disambiguation/documentation.

### 20.5 Literal suffixes

Domains may bind literal suffixes.

Example:

```aivi
domain Duration over Int
    literal ms  : Int -> Duration
    literal sec : Int -> Duration
    literal min : Int -> Duration
```

This enables:

```aivi
val a:Duration = 250ms
val b:Duration = 10sec
val c:Duration = 3min
```

Literal-suffix rules:

- domain suffix names in v1 must be at least two ASCII letters long
- single-letter alphabetic suffixes are reserved for built-in numeric literal families and future
  core numeric extensions
- compact `digits + suffix` is a suffix literal candidate; spaced forms such as `250 ms` are
  ordinary application
- the current surface only supports integer-family domain suffix literals
- suffix resolution is compile-time only
- suffix resolution is against matching domain literal declarations in the **current module only**
- no match is an error; more than one current-module match is an ambiguity error
- imported modules do not extend the literal-suffix search space in the current slice

Examples:

- `250ms : Duration`
- `250 : Int`
- `250ms + 3min` is legal only if `Duration` defines `+`
- `250ms + 3` is illegal unless an explicit constructor or operator admits it

### 20.6 Domain operators

Domains may define a restricted set of domain-local operators.

Example:

```aivi
domain Duration over Int
    literal ms : Int -> Duration
    (+)        : Duration -> Duration -> Duration
    (-)        : Duration -> Duration -> Duration
    (*)        : Duration -> Int -> Duration
    compare    : Duration -> Duration -> Ordering
```

Example:

```aivi
domain Path over Text
    (/) : Path -> Text -> Path
```

Operator rules:

- operator resolution is static
- operators are not inherited from the carrier automatically
- operators must be declared by the domain or provided by explicit class evidence over the domain
- operators are type-checked before any fallback inference logic
- proven domain operators cross an explicit elaboration seam into typed core/backend; later layers do
  not rediscover them heuristically

### 20.7 Smart construction and invariants

Domains are the preferred place to attach invariants that are stronger than the carrier type.

Examples:

- `Url over Text` may require URL parsing
- `Path over Text` may normalize separators
- `Color over Int` may require packed ARGB layout
- `NonEmpty A over List A` may reject empty lists

Example:

```aivi
domain NonEmpty A over List A
    fromList : List A -> Option (NonEmpty A)
    head     : NonEmpty A -> A
    tail     : NonEmpty A -> List A
```

The carrier type alone does not imply the invariant. The domain does.

### 20.8 Parameterized domains

Domains may be parameterized in the same style as ordinary type constructors.

Example:

```aivi
domain ResourceId A over Text
domain NonEmpty A over List A
```

Typing rules:

- parameters are ordinary type parameters
- kinds follow the ordinary kind system
- the carrier may use those parameters
- partial application of parameterized domains is allowed when the resulting kind matches the
  expected constructor kind

### 20.9 Equality and instances

A domain does not automatically inherit all instances of its carrier.

Recommended v1 rule:

- `Eq` may be compiler-derived for a domain if its carrier has `Eq` and the domain does not opt out
- domain identity is preserved even when equality is derived from the carrier's structure
- other class evidence is explicit unless separately declared

### 20.10 Runtime representation

A domain reuses its carrier runtime representation unless a later lowering layer documents a more
specialized ABI. The nominal distinction is preserved in typing and diagnostics.

### 20.11 No implicit casts

Domains do not introduce implicit coercions to or from the carrier.

### 20.12 Diagnostics

Diagnostics should name the domain rather than erasing it to the carrier.

For literal/decode/operator failures, diagnostics should explain whether the failing surface was:

- unresolved suffix lookup
- ambiguous suffix or decode surface
- illegal raw-carrier use where a domain value was required
- missing domain operator or parser surface

### 20.13 Recommended examples

#### Duration

```aivi
domain Duration over Int
    literal ms  : Int -> Duration
    literal sec : Int -> Duration
    value       : Duration -> Int
    (+)         : Duration -> Duration -> Duration
```

#### Url

```aivi
domain Url over Text
    parse : Text -> Result UrlError Url
    value : Url -> Text
```

#### Path

```aivi
domain Path over Text
    value : Path -> Text
    (/)   : Path -> Text -> Path
```

#### NonEmpty

```aivi
domain NonEmpty A over List A
    fromList : List A -> Option (NonEmpty A)
    head     : NonEmpty A -> A
    tail     : NonEmpty A -> List A
```

### 20.14 Design boundary for v1

The implemented v1 domain slice is intentionally narrow:

- declarations, callable members, explicit construction/unwrapping, explicit decode surfaces, and
  domain-local operators are in scope
- literal suffixes are current-module integer-family surfaces only
- no implicit casts
- no projection syntax
- literal patterns remain on the existing integer/text-only slice rather than widening pattern
  matching around domain literals prematurely

---

## 21. Diagnostics

AIVI favors explicitness over clever inference.

Diagnostics must:

- identify the failed invariant
- point at the user-visible cause
- avoid leaking backend IR details unless requested in debug output
- suggest the intended construct when the misuse is obvious

Examples:

- using a signal in `val` should suggest `sig`
- omitting a record field without a `Default` instance should name the missing field and missing instance
- mixing applicative constructors in one `&|>` cluster should report the first mismatch and the expected common outer constructor

---

## 22. Formatter

The formatter is part of the language contract.

### 22.1 Formatter goals

- canonical pipe alignment
- canonical arrow alignment in contiguous match arms
- stable formatting for records, markup, and clustered applicative spines

### 22.2 `&|>` formatting

The formatter should preserve and prefer the leading-cluster style when the spine is vertically scanned for independence.

Preferred example:

```aivi
sig validatedUser =
 &|> validateName nameText
 &|> validateEmail emailText
 &|> validateAge ageText
  |> UserDraft
```

This is a first-class canonical style, not a tolerated edge case.

---

## 23. Testing and hardening

AIVI requires aggressive implementation hardening. The baseline implementation strategy includes:

- parser and decoder fuzzing in a standalone top-level `fuzz/` cargo-fuzz workspace
- stable corpus replay tests in ordinary CI so committed seeds are checked without requiring the
  `cargo fuzz` subcommand
- scheduler stress coverage that stays deterministic and in-process
- GTK subtree lifecycle tests
- stack-depth torture tests
- teardown/leak tests
- deterministic scheduling tests with generation-stamped publication scripts
- GLib wakeup and reentry tests

Decoder fuzzing is schema-owned rather than arbitrary-runtime-string fuzzing. The fuzz target first
parses and lowers source text, then executes only compiler-generated decode programs. Malformed
inputs may fail, but failures must flow through typed decode errors whose field and variant names
come from the generated schema rather than ad hoc runtime strings.

Performance work is benchmark-gated. Each performance-oriented pass should land with:

- one checked-in corpus
- one machine-stable structural metric
- one release timing metric

Anecdotal speedups do not count.

Every bug fix should add a regression test that names the failed invariant.

---

## 24. Milestones

These milestones do **not** reduce scope. They partition implementation work.

Status legend: **COMPLETE** = fully implemented; **PARTIAL** = core slice implemented with known
gaps; **PENDING** = not yet started.

### Milestone 1 — Surface and CST freeze — **COMPLETE**

- lexer ✓
- parser ✓
- CST (lossless for formatting and diagnostics) ✓
- formatter (canonical pipe, arrow, cluster alignment) ✓
- syntax for `type`, `class`, `instance`, `val`, `fun`, `sig`, `use`, `export`, `provider`, markup,
  and pipe operators ✓
- line/block/doc comment lexing (`//`, `/* */`, `/** **/`) and trivia retention in the token stream ✓
- regex literal lexing plus HIR validation ✓
- compact suffix literal lexing (`250ms`) ✓

### Milestone 2 — HIR and names — **COMPLETE**

- name resolution ✓
- import resolution ✓
- import alias (`use module (x as y)`) ✓
- decorator attachment (`@source`, `@recur.timer`, `@recur.backoff`) ✓
- explicit HIR nodes for applicative clusters and markup control nodes ✓
- domain declarations and suffix namespaces ✓
- `instance` blocks with same-module class resolution ✓
- provider declarations (`provider qualified.name`) ✓
- input signal declarations (body-less annotated `sig`) ✓
- module-aware expression typechecker in `aivi-hir` ✓

### Milestone 3 — Kinds and core typing — **COMPLETE**

- kind checking ✓
- class/instance resolution and evidence ✓
- constructor partial application ✓
- `Validation` ✓
- `Default` and record default elaboration ✓
- `Eq` compiler derivation ✓
- operator typechecking (`==`, `!=`, domain operators) ✓
- truthy/falsy branch handoff (`T|>`, `F|>`) including one-layer `Signal` lift ✓
- case exhaustiveness checks for known closed sums ✓
- bidirectional record/collection/projection shape checking ✓

### Milestone 4 — Pipe normalization — **COMPLETE**

- exact `&|>` normalization into applicative spines ✓
- recurrence node representation ✓
- recurrence scheduler-node handoff ✓
- gate (`?|>`) lowering plan ✓
- fan-out (`*|>` / `<|*`) typed handoff ✓
- source lifecycle handoff ✓
- diagnostics for illegal unfinished clusters ✓

### Milestone 5 — Reactive core and scheduler — **COMPLETE**

- signal graph extraction ✓
- topological scheduling with GLib main-context integration ✓
- transactional ticks with generation stamps ✓
- deterministic propagation with stale-publication rejection ✓
- cancellation/disposal and owner-liveness tracking ✓
- GLib cross-thread wakeup with reentry guard ✓

### Milestone 6 — Tasks and sources — **PARTIAL**

- `Task` typed IR and scheduler completion ports ✓
- `@source` runtime contract and instance lifecycle ✓
- decode integration (structural decoder, domain parse method resolution) ✓
- worker/UI publication boundary ✓
- timer sources (`timer.every`, `timer.after`) — fully working ✓
- HTTP sources — runtime contract wired, provider execution slice partial
- `fs.read`, `fs.watch` — contract wired, provider execution slice partial
- socket / mailbox / process / D-Bus / window-event sources — partial or provider-specific
- full recurrent-task execution — pending

### Milestone 7 — GTK bridge — **PARTIAL**

- widget plan IR ✓
- runtime assembly ✓
- GTK bridge graph and child-group lowering ✓
- executor with direct setter/event/child management ✓
- `<show>` / `keepMounted` ✓
- `<each>` with required keys and localized child edits ✓
- `<empty>` ✓
- `<match>` ✓
- `<fragment>` ✓
- `<with>` ✓
- widget schema metadata for the current live widget surface ✓
- full widget property catalog — pending

### Milestone 8 — Backend and hardening — **PARTIAL**

- lambda IR with explicit closures and environments ✓
- backend IR with layouts, kernels, pipelines, source plans, and decode plans ✓
- Cranelift AOT codegen for scalars and item-body kernels ✓
- runtime startup linking (HIR → backend → scheduler) ✓
- inline helper pipe execution in item/source kernels ✓
- body-backed signal inline transform/tap/case/truthy-falsy execution against committed snapshots ✓
- general lambda/closure conversion for arbitrary bodies — pending
- scheduler-owned signal filter/fanout/recurrence pipeline execution — pending
- initial moving-GC integration — pending
- fuzzing and deterministic stress infrastructure — in progress
- performance pass plan frozen and benchmark-gated (see §28.8–§28.9)

---

## 25. Bottom-line implementation guidance

AIVI should be implemented as one coherent system:

- typed and lowered through explicit IR boundaries
- stack-safe by design
- scheduler-driven and deterministic
- pure in the language core
- explicit at all effect boundaries
- GTK-first without collapsing into callback-driven impurity
- direct-binding-oriented, not virtual-DOM-oriented

The implementation should prefer one correct algebraic model over many local patches. In particular:

- `&|>` must remain one applicative story across `Validation`, `Signal`, `Option`, `Result`, and `Task`
- record omission must remain explicit-default completion, not open records
- `Task` must remain the only user-visible one-shot effect carrier
- GTK markup must lower directly and predictably to widgets, setters, handlers, and child management
---

## 26. CLI reference

The `aivi` CLI provides the following subcommands.

Unless otherwise stated, multi-file module discovery uses the nearest ancestor `aivi.toml`; if no
such file exists, the entry file's parent directory is the workspace root. Module names come from
relative `.aivi` paths under that root.

### 26.1 `aivi check <path>`

Validates an AIVI source file through the honest frontend/compiler boundary:

```
aivi check src/main.aivi
```

Pipeline: source → CST → HIR → typed core → lambda → backend (no code emission).

Reports diagnostics with source locations. Exits 0 if there are no errors, 1 if there are errors,
2 on internal failure.

Invoking `aivi <path>` with no subcommand is equivalent to `aivi check <path>`.

### 26.2 `aivi compile <path> [-o <output>]`

Compiles an AIVI source file to a native object file:

```
aivi compile src/main.aivi -o build/main.o
aivi compile src/main.aivi --output build/main.o
```

Pipeline: source → CST → HIR → typed core → lambda → backend → Cranelift → object file.

If `-o` / `--output` is omitted, no output file is written but the pipeline is validated. Exits 0
on success, 1 on compilation errors.

`aivi compile` stops at the honest compile boundary. Runtime startup, final linking, and executable
launch are reported as separate work rather than being implied.

### 26.3 `aivi run <path> [--view <name>]`

Compiles and runs an AIVI module as a GTK application:

```
aivi run src/app.aivi
aivi run src/app.aivi --view mainWindow
```

View selection rules:

1. If `--view <name>` is given, the named top-level markup-valued `val` is used.
2. Otherwise, if there is a top-level markup-valued `val` named `view`, that is used.
3. Otherwise, if there is a unique top-level markup-valued `val`, that is used.
4. Otherwise, `--view <name>` is required.

The selected root must be a `Window`. The CLI does not auto-wrap arbitrary widgets into windows.

`aivi run` owns the live GTK/runtime session. It links the compiled runtime stack, evaluates the
selected view fragments against committed runtime snapshots, re-evaluates those fragments after each
meaningful committed tick, and applies GTK updates through the bridge executor.

Exits 0 on clean application close, 1 on startup/compilation error.

### 26.4 `aivi fmt [--stdin | --check] [<path>...]`

Formats AIVI source files:

```
aivi fmt src/app.aivi             # format to stdout
aivi fmt --stdin                  # read from stdin, write to stdout
aivi fmt --check src/a.aivi src/b.aivi   # verify formatting; exit 1 if any differ
```

The formatter is canonical: it produces a single deterministic output for any valid source.
Formatting is part of the language contract (§22).

### 26.5 `aivi lex <path>`

Tokenizes an AIVI source file and prints the token stream:

```
aivi lex src/app.aivi
```

Useful for debugging lexer behavior, regex literal handling, or suffix literal resolution.

### 26.6 `aivi lsp`

Starts the AIVI Language Server on stdin/stdout using the Language Server Protocol:

```
aivi lsp
```

Editor integrations should launch this subprocess and communicate over stdio. See §27 for
supported LSP capabilities.

### 26.7 `aivi db migrate`

Generates a plain SQL migration from the current record-schema surface:

```
aivi db migrate
```

The command diffs the current record types against the last applied migration state and writes a new
SQL file under `db/migrations/` with a timestamp-prefixed filename. The generated file is ordinary
SQL intended for review and commit.

### 26.8 `aivi db apply`

Applies pending SQL migrations:

```
aivi db apply
```

Pending files are applied in lexicographic order using a `_schema_migrations` tracking table inside
one transaction. On failure, the whole migration application rolls back.

---

## 27. Language server (LSP)

The AIVI language server (`aivi lsp`) provides editor integration through the Language Server
Protocol. It is backed by the `aivi-query` incremental query database, which caches source, parse,
HIR, diagnostic, symbol, and format results per revision.

### 27.1 Supported capabilities

| Capability | Status |
|---|---|
| Text document sync (full) | ✓ |
| Diagnostics (publish on open/change) | ✓ |
| Document formatting | ✓ |
| Document symbols | ✓ |
| Workspace symbols | Partial |
| Hover documentation | ✓ |
| Go-to-definition | ✓ |
| Completion (triggered on `.`) | ✓ |
| Semantic tokens (full) | Partial |

### 27.2 Architecture

The LSP server is read-only from the user model's point of view. All editor features go through the
revision-keyed query database rather than invoking ad-hoc frontend passes.

When a workspace root is known, the server uses the same `aivi.toml` / relative-path module mapping
as the CLI. Incremental memoization is per file revision so rapid keystroke changes do not
invalidate unrelated cached queries.

### 27.3 Current limitations

- whole-workspace semantic queries remain partial; the checked/open file set is still the primary
  working set for symbols and diagnostics
- completion suggestions are basic; richer type-directed completion over expected record fields and
  constructor arguments is pending
- semantic token legend exists but token-type coverage is still incomplete
- editor-facing project orchestration does not yet replace the explicit CLI workflow for runtime,
  migrations, or provider startup validation

---

## 28. Pre-stdlib runtime and application surfaces

This section records implementation-facing surfaces that are real parts of the current runtime slice
but are not yet a polished standard library.

### 28.1 Workspace and module discovery

Multi-file workspace discovery is shared across `check`, `compile`, and `run`:

- the nearest ancestor `aivi.toml` is the workspace root when present
- otherwise the entry file's parent directory is the root
- module names come from relative `.aivi` paths under that root
- all commands must agree on this mapping

### 28.2 Database schema and migrations

The database surface treats AIVI record types as the schema source of truth.

Rules:

- migrations are CLI-generated SQL files, not an AIVI-specific migration DSL
- generated migrations live under `db/migrations/`
- runtime startup checks that the applied migration state matches the schema version the program was
  compiled against
- if the versions differ, startup fails with `DbError.SchemaMismatch` before any query runs
- there is no auto-migration in production

### 28.3 D-Bus surface

The user-facing D-Bus surface in the current slice is intentionally narrow and explicit:

- `dbus.ownName` is a `@source` for name ownership state
- `dbus.call` is a `Task`
- `dbus.emit` is a `Task`
- `dbus.signal` is a `@source` for inbound signal subscription
- `dbus.method` is a `@source` for fire-and-forget inbound method dispatch with immediate Unit
  reply semantics on the wire

Methods that return non-Unit values to the caller are deferred.

### 28.4 Local-first sync architecture

The reference email-oriented runtime shape is local-first:

- IMAP sync runs on a worker and writes fetched mail into SQLite through the database layer
- the UI reads via `db.query` over the local database rather than binding directly to a live IMAP
  stream
- the sync source publishes typed `SyncState`
- credential errors surface in `SyncState.error` and do not permanently tear down the source on the
  first auth failure
- SMTP send is a separate one-shot `Task SmtpError Unit`

### 28.5 Multi-process desktop architecture

The intended cooperating-process shape for the reference desktop application is:

- a headless sync daemon
- a GTK UI process
- a GJS GNOME Shell extension

The daemon owns the D-Bus well-known name and SQLite write lock. The UI reads through SQLite and
subscribes to daemon D-Bus signals. The extension communicates with the daemon through D-Bus only.
SQLite WAL mode covers daemon writes plus UI reads.

Closing the main window with `hideOnClose=True` hides the window rather than terminating the live
process. The existing instance is later restored by presentation or D-Bus activation.

### 28.6 Moving-GC rollout boundary

The initial moving-GC boundary is deliberately narrow:

- only scheduler-committed runtime snapshots are in moving-GC storage
- pending worker/source/task/evaluator results remain ordinary Rust-owned runtime values until
  commit
- GTK, worker, and provider seams keep explicit detached boundary wrappers so later GC expansion can
  happen without reopening those contracts

### 28.7 Runtime startup and linked ownership

Runtime startup links HIR runtime bindings to backend items, source kernels, and widget fragments.
The long-lived linked runtime owns its compiled backend program behind shared ownership suitable for
persistent GLib-driven sessions.

### 28.8 Hardening requirements

Hardening remains deterministic and in-process:

- scheduler stress uses the existing runtime/unit harnesses rather than a separate async test stack
- teardown, wakeup, and reentry behavior must be testable without sleep-driven flakiness
- parser and decoder fuzzing live in the standalone `fuzz/` workspace described in §23

### 28.9 Performance gate policy

Performance passes start only after typed-core validation. First-wave scope is intentionally narrow:

- typed-lambda capture pruning
- backend kernel simplification
- direct self-tail loop lowering
- scheduler frontier deduplication

HIR, typechecking, and typed core remain proof and diagnostic layers rather than speculative
performance layers. Every performance pass must satisfy the benchmark gate policy in §23.