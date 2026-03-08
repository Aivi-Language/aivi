# Minimality (informal)

AIVI has a rich surface language, but the compiler lowers it to a much smaller core. This is useful for both implementers and advanced users: once lowering is complete, the backend only needs to understand a compact set of building blocks.

This page is mainly for contributors and curious advanced readers. The point is not that AIVI source code is tiny; it is that many different source features end up using the same small implementation core.

If you want the surrounding pipeline first, start with [Compiler & Backend Design](design.md).

A few terms used on this page:

- **λ** means an anonymous function
- **currying** means representing a multi-argument function as a chain of one-argument functions
- **`let rec`** means a recursive binding in the core language
- **HKT** means a higher-kinded type, a type constructor that itself takes type parameters
- **Church-encoded** means representing a structure by the function that consumes it instead of by a dedicated runtime object
- **dictionary** means a record of class methods that the compiler can pass implicitly

## Surface feature → kernel building block

This table mixes true kernel primitives with a few compile-time rewrites and backend-retained forms. The point is that each surface feature stops introducing new semantic machinery once lowering is complete.

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
| Generic do-monads | `chain` + `of` (`do Query` uses `queryChain` + `queryOf`) |
| Resources | `__makeResource` |
| Plain blocks | immediately-applied λ |
| Domains | static rewrite to ordinary calls or templates |
| Sigils | specialized lowering (raw sigils stay literals; structured sigils become ordinary expressions) |
| Modules | namespaces |
| Classes | instance dictionaries (records) |
| HKTs | ∀ |

In other words, the surface language is expressive, but the implementation core is intentionally small. Some features disappear as rewrites before the kernel proper, while others survive briefly as backend-oriented literal forms, but they still avoid adding new expressive power.

## The kernel in one sentence

AIVI's kernel is lambda calculus (a tiny mathematical model of functions) with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.

That means features that look quite different in source code—`do Effect`, `resource`, `generate`, domains, or predicates—end up being lowered to the same small set of primitives.

## How block forms are lowered

These forms do not all disappear in the same pass. Generic `do M` blocks other than `do Effect` are rewritten during the surface → HIR lowering step; `do Effect`, `resource`, `generate`, and `plain` blocks are rewritten later by `desugar_blocks()` during the HIR → block-free HIR step.

The combined result is block-free HIR composed of nested lambdas and ordinary function calls.

| Block kind | Stage | Desugared form |
|:---------- |:----- |:-------------- |
| `do Option { x <- e1; e2 }` | Surface → HIR | `chain (λx → e2) e1` |
| `do Result { x <- e1; e2 }` | Surface → HIR | `chain (λx → e2) e1` |
| `do Query { x <- e1; e2 }` | Surface → HIR | `queryChain (λx → e2) e1` |
| `do Effect { x <- e1; e2 }` | HIR → block-free HIR | `__withResourceScope (bind e1 (λx → e2))` |
| `do Effect { x = e; rest }` | HIR → block-free HIR | `__withResourceScope (bind (pure e) (λx → rest))` |
| `resource { acq; yield v; cleanup }` | HIR → block-free HIR | `__makeResource (λ_ → acquire_chain) (λ_ → cleanup_chain)`, where each chain is lowered with the `do Effect` rules |
| `generate { yield e; ... }` | HIR → block-free HIR | Church-encoded fold via `gen_bind`, `gen_yield`, and `gen_append` (the generator is represented by the fold that consumes it, not by a separate runtime generator data type) |
| `plain { x = e; body }` | HIR → block-free HIR | `(λx → body) e` |

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

## See also

- [Compiler & Backend Design](design.md) for the full compilation pipeline
- [Generic `do M` Blocks](../syntax/do_notation.md) and [Effects](../syntax/effects.md) for the surface rules behind the block examples
- [Domains & Units](../syntax/domains.md), [Operators and Context](../syntax/operators.md), and [Classes and Higher-Kinded Types](../syntax/types/classes_and_hkts.md) for the surface features summarized here
