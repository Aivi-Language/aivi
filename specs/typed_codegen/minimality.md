# Minimality (informal)

AIVI has a rich surface language, but the compiler lowers it to a much smaller core. This is useful for both implementers and advanced users: once lowering is complete, the backend only needs to understand a compact set of building blocks.

## Surface feature → kernel building block

| Surface feature | Kernel primitive |
|:--------------- |:---------------- |
| Lambdas | λ |
| Multi-argument functions | currying |
| Recursion | `let rec` |
| Patterns | `case` |
| Records | closed records + update |
| Patching | update + fold |
| Predicates | λ + case |
| Generators | fold (Church-encoded) |
| Effects (`do Effect {}`) | `bind` + `pure` |
| Generic do-monads | `chain` + `of` |
| Resources | `__makeResource` |
| Plain blocks | immediately-applied λ |
| Domains | static rewrite |
| Sigils | function call |
| Modules | namespaces |
| Classes | dictionaries (records) |
| HKTs | ∀ |

In other words, the surface language is expressive, but the core it compiles to is intentionally small.

## The kernel in one sentence

AIVI's kernel is λ-calculus with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.

That means features that look quite different in source code—`do Effect`, `resource`, `generate`, domains, or predicates—end up as elaborations of the same small set of primitives.

## How block forms are lowered

Generic do-monads (`do Option`, `do Result`, `do List`) and effect blocks are desugared by a `desugar_blocks()` pass that transforms `HIR -> HIR`.
The output is block-free HIR composed of nested lambdas and ordinary function calls.

| Block kind | Stage | Desugared form |
|:---------- |:----- |:-------------- |
| `do Option { x <- e1; e2 }` | HIR transform | `chain e1 (λx → e2)` |
| `do Result { x <- e1; e2 }` | HIR transform | `chain e1 (λx → e2)` |
| `do Effect { x <- e1; e2 }` | HIR transform | `__withResourceScope(bind e1 (λx → e2))` |
| `do Effect { x = e; rest }` | HIR transform | `bind (pure e) (λx → rest)` |
| `resource { acq; yield v; cleanup }` | HIR transform | `__makeResource (λ_ → acq_chain) (λ_ → cleanup_chain)` |
| `generate { yield e; ... }` | HIR transform | Church-encoded fold via `gen_bind`, `gen_yield`, and `gen_append` |
| `plain { x = e; body }` | HIR transform | `(λx → body) e` |

By the time lowering reaches RustIR, these block forms no longer exist as separate syntax categories.

## Why some non-kernel variants are kept

Several RustIR expression forms could be reduced further to the pure kernel, but AIVI keeps them because they produce much better native code.

| Variant | Theoretical reduction | Why it is kept |
|:------- |:--------------------- |:-------------- |
| `If` | `Match` on `Bool` | Cranelift can emit a single branch instruction instead of a full match lowering |
| `Binary` | `App(App(Var(op), left), right)` | Known numeric operators lower to native int/float instructions |
| `Call` | Nested `App` | Avoids building intermediate closures for multi-argument calls |
| `Pipe` | `App(func, arg)` | Preserves debug metadata such as `pipe_id`, step index, and label |
| `TextInterpolate` | Chain of string concatenations | Allows more efficient string assembly |

These variants do not add expressive power. They are retained because the backend can generate better code from them than it could from their fully reduced equivalents.
