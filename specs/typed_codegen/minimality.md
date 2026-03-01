# Minimality proof (informal)

| Feature                  | Kernel primitive                                  |
|:------------------------ |:------------------------------------------------- |
| Lambdas                  | λ                                                 |
| Multi-arg functions      | currying                                          |
| Recursion                | `let rec`                                         |
| Patterns                 | case                                              |
| Records                  | closed records + update                           |
| Patching                 | update + fold                                     |
| Predicates               | λ + case                                          |
| Generators               | fold (Church-encoded)                             |
| Effects (`do Effect {}`) | `bind` + `pure` (nested λ / application)          |
| Generic do-monads        | `chain` + `of` (desugared at HIR level)           |
| Resources                | `__makeResource` (λ-wrapped acquire/cleanup pair) |
| Plain blocks             | immediately-applied λ                             |
| Domains                  | static rewrite                                    |
| Sigils                   | function call                                     |
| Modules                  | namespaces                                        |
| Classes                  | dictionaries (records)                            |
| HKTs                     | ∀                                                 |

Nothing else is required.

## The true kernel

> **AIVI's kernel is simply:**
> **λ-calculus with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.**
> **All block forms — `do Effect {}`, `do Option {}`, `do Result {}`, `resource {}`, `generate {}`, and `plain {}` — are fully desugared into nested λ/application/`bind`/`pure`/`chain`/`of` within the HIR itself.**
> **Domains are static rewrite rules; patching, predicates, generators, resources, and effects are all elaborations of these primitives.**

## Block desugaring

Generic do-monads (`do Option`, `do Result`, `do List`) and Effect blocks are desugared
via a `desugar_blocks()` pass that operates `HIR -> HIR`. This transformation
eliminates all `Block`, `BlockKind`, and `BlockItem` variants, producing a flat HIR
composed of nested lambdas and function calls.

| Block kind                           | Stage         | Desugared form                                                    |
|:------------------------------------ |:------------- |:----------------------------------------------------------------- |
| `do Option { x <- e1; e2 }`          | HIR transform | `chain e1 (λx → e2)` — generic monadic desugaring                 |
| `do Result { x <- e1; e2 }`          | HIR transform | `chain e1 (λx → e2)` — generic monadic desugaring                 |
| `do Effect { x <- e1; e2 }`          | HIR transform | `__withResourceScope(bind e1 (λx → e2))`                          |
| `do Effect { x = e; rest }`          | HIR transform | `bind (pure e) (λx → rest)` — let-binds are `pure`-wrapped at HIR |
| `resource { acq; yield v; cleanup }` | HIR transform | `__makeResource (λ_ → acq_chain) (λ_ → cleanup_chain)`            |
| `generate { yield e; ... }`          | HIR transform | Church-encoded fold via `gen_bind`/`gen_yield`/`gen_append`       |
| `plain { x = e; body }`              | HIR transform | `(λx → body) e` — immediately-applied lambda                      |

The resulting HIR is block-free before it reaches `RustIr` lowering.

## Optimization variants

Several `RustIrExpr` variants are theoretically reducible to the core λ-calculus +
case primitives, but are preserved through to the Cranelift backend for code quality:

| Variant           | Theoretical reduction                | Why it is kept                                                        |
|:----------------- |:------------------------------------ |:--------------------------------------------------------------------- |
| `If`              | `Match` on `Bool` (`True`/`False`)   | Cranelift emits a single `brif` branch vs. full match compilation     |
| `Binary`          | `App(App(Var(op), left), right)`     | Cranelift uses native int/float machine instructions when types known |
| `Call`            | Nested `App` (curried application)   | Avoids intermediate closure allocation for multi-arg calls            |
| `Pipe`            | `App(func, arg)`                     | Carries debug metadata (`pipe_id`, `step`, `label`) for `@debug`      |
| `TextInterpolate` | Chain of `++` (string concatenation) | Cranelift builds interpolated strings in a single pass                |

These variants do not add expressive power — every program using them can be
rewritten using only λ, application, and case. They exist because the Cranelift
backend produces significantly better native code for these common patterns.
