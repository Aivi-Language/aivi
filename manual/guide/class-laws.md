# Class Laws & Design Boundaries

Classes are not just name lookup. Each class advertises a semantic contract.

The compiler checks declarations, instance heads, and evidence selection. It does **not** prove that an
instance is lawful. When you publish an instance, you are asserting the laws and guidance below.

For the current builtin executable support matrix, see
[Typeclasses & Higher-Kinded Support](/guide/typeclasses#canonical-builtin-executable-support).

## Equality and ordering

| Class | Law / guidance |
| --- | --- |
| `Eq A` | Equality should be reflexive, symmetric, and transitive. Surface `!=` uses the same `Eq` evidence as `==`; document and reason about equality itself, not a second independent notion. |
| `Ord A` | `compare` should agree with `Eq` and define a total order: every pair is comparable, ordering is antisymmetric, and ordering is transitive. The ordinary `<`, `>`, `<=`, and `>=` operators are just surface forms of this `compare`. |

Comparison capability follows the executable member's type and class identity, including through
aliases and re-exports. An authored equality or ordering class assumes the same law obligations
when its members back these operators. See [Comparison evidence](/guide/typeclasses#comparison-evidence)
for the required signatures, dictionary precedence, and ambiguity behavior.

## Higher-kinded hierarchy

| Class | Law / guidance |
| --- | --- |
| `Functor F` | Identity and composition: mapping `id` changes nothing, and mapping a composition is the same as composing two maps. |
| `Apply F` | Applicative-style composition: function application inside the carrier must compose the same way ordinary function application does. |
| `Applicative F` | Identity, homomorphism, interchange, and composition. `pure` must inject a plain value without changing the surrounding effect story. |
| `Chain M` | Associativity: sequential dependent steps must regroup without changing the result. |
| `Monad M` | Left identity, right identity, and associativity. `join` and `chain` must describe the same dependent sequencing. |
| `Foldable F` | `reduce` must visit elements in the carrier's declared order and treat empty and singleton shapes consistently. |
| `Traversable T` | Traversal must preserve shape and satisfy the standard identity, naturality, and composition laws. Use it when you are sequencing effects through a fixed structure, not when you are changing the structure itself. |
| `Filterable F` | `filterMap` may drop or rewrite existing positions, but it must not reorder or duplicate them. `filterMap Some` should behave like the identity. |
| `Bifunctor F` | Identity and composition in both arguments: `bimap id id` is `id`, and mapping both sides composes pointwise. |
| `Semigroupoid P` | Associativity: `compose f (compose g h)` and `compose (compose f g) h` must agree for every compatible sequence of input and output types. |
| `Category P` | Inherits associative composition. Left and right identity require `compose id f` and `compose f id` to agree with `f`. |
| `Profunctor P` | `dimap id id` preserves the value. Input transformations compose contravariantly and output transformations compose covariantly. |

## Why `Signal` stops at `Applicative`

`Signal` is intentionally lawful as `Functor`, `Apply`, and `Applicative`, but not as `Chain` or
`Monad`.

The reason is architectural, not cosmetic: AIVI extracts a static signal dependency graph, schedules it
topologically, and keeps propagation glitch-free per tick. A monadic `Signal` would imply data-dependent
dependency rewiring, which would blur graph extraction, scheduler ownership, teardown, and diagnostics.
So `&|>` is the correct abstraction for combining independent signals; dependent reactive rewiring is
not modelled as class-backed `bind`.

## Why `Validation` stops at `Applicative`

`Validation E` is intentionally lawful as an accumulation-oriented `Applicative`, not as `Chain` or
`Monad`.

Independent checks belong in applicative composition because all failures should be reported together.
In AIVI that is the role of `&|>`. Dependent checks are a different
story: use the dedicated `!|>` pipe surface or explicit helpers such as `aivi.validation.andThen` when
later work depends on earlier success. Keeping that split explicit prevents “accumulating monad”
confusion and preserves the mathematical story of `Validation`.

## Why `Task` keeps `Chain` and `Monad`

`Task E` is a one-shot computation description, so it supports both independent applicative combination
and dependent monadic sequencing.

- Use `&|>` when the tasks are independent and you are assembling a pure result from both.
- Use `chain` / `join` / monadic style when the second task depends on the first task's value.

That division is why the current builtin executable support includes `Functor`, `Apply`,
`Applicative`, `Chain`, and `Monad` for `Task E`.

`traverse` can sequence a fixed structure into `Task E` through its `Applicative` dictionary.
Constructing that traversal creates a plan; it does not perform the task effects. The plan preserves
the structure's declared element order, and empty structures are lifted with `pure`.

The stdlib traversals follow the same contract: `Either E` visits only `Right`, `Dict K` visits
values in entry order without comparing keys, `NonEmptyList` visits head to tail, and `Matrix`
visits cells in row-major order. Successful traversals preserve keys, element positions and
dimensions; an outer applicative may fail or produce several alternatives without changing the
shape of any successful result. Executable validation accumulation uses `Validation (NonEmptyList E)`.

`aivi execute`, `aivi test`, linked task workers, `db.live`, and D-Bus reply-task workers retain
the task's code and globals while running deferred callbacks. Task application runs the function-producing task before
the value-producing task; a failure stops the remaining effects. Composition is driven by an
explicit continuation stack, so nesting `map`, `apply`, `chain`, or `join` does not consume one
Rust call frame per task. Host integrations that execute a composed task must supply an
execution environment; the bare Rust executor reports an error when one is required.

Database query and commit plans use the same task carrier. Composition receives
the success value directly, and SQL failures stop pending callbacks and effects.
Each successful database transaction sends its reactive change notification even
when a later task step fails.

## Practical rule

If you are unsure whether a class is the right abstraction, ask what must stay fixed:

- fixed structure, effects threaded through it → `Traversable`
- independent effects combined in parallel structure → `Applicative` / `&|>`
- later work depends on earlier values → `Chain` / `Monad`
- reactive graph must stay static → stop at `Signal` applicative
- validation must accumulate all independent failures → stop at `Validation` applicative
