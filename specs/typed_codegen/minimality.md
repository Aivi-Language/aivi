# Why the Surface Language Can Stay Small

AIVI deliberately offers a rich surface syntax while still lowering to a small semantic core. This page sketches that minimality argument for readers who want to understand why flows, patches, predicates, and domain features do not require a huge kernel.

## Surface features versus kernel primitives

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
| Flat flows (`|>`, `>|>`, `?|>`, `!|>`) | `bind` / `pure` / `attempt` + case |
| Applicative siblings (`&|>`) | `map` + `ap` |
| Fan-out (`*|>` ... `*-|`) | fold (Church-encoded) |
| Cleanup (`@cleanup`) | scope-registered finalization |
| Plain blocks | immediately-applied λ |
| Domains | static rewrite to ordinary calls or templates |
| Sigils | specialized lowering (raw sigils stay literals; structured sigils become ordinary expressions) |
| Modules | namespaces |
| Classes | instance dictionaries (records) |
| HKTs | ∀ |

In other words, the surface language is expressive, but the implementation core is intentionally small. Some features disappear as rewrites before the kernel proper, while others survive briefly as backend-oriented literal forms, but they still avoid adding new expressive power.

## The kernel in one sentence

AIVI's kernel is lambda calculus (a tiny mathematical model of functions) with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.

That means features that look quite different in source code—flat flows, cleanup registration, fan-out, domains, or predicates—end up being lowered to the same small set of primitives.

## How flow forms are lowered

These forms do not all disappear in the same pass. Some are normalized during surface → HIR lowering, while others survive until later desugaring passes that erase flow structure and explicit cleanup scopes.

| Surface form | Stage | Desugared form |
|:------------ |:----- |:-------------- |
| `seed |> step1 |> step2` | Surface → HIR | nested `bind` / map over the current carrier |
| `seed ?|> risky !|> Pat => recover` | Surface → HIR | `attempt risky` plus pattern-based recovery |
| `input &|> a #x &|> b #y |> body` | Surface → HIR | applicative `map` / `ap` chain |
| `items *|> ... *-|` | HIR → block-free HIR | Church-encoded fold / list builder |
| `line @cleanup cleanup` | HIR → block-free HIR | register finalizer around the lowered successful result |
| `seed @|> retry ... recurse retry` | Surface → HIR | local loop / restart structure |
| `plain { x = e; body }` | HIR → block-free HIR | `(λx → body) e` |

By the time lowering reaches RustIR, these flow-oriented surface forms no longer exist as separate syntax categories.

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
- [Flow Syntax](../syntax/flows.md) and [Effects](../syntax/effects.md) for the current workflow surface
- [Domains & Units](../syntax/domains.md), [Operators and Context](../syntax/operators.md), and [Classes and Higher-Kinded Types](../syntax/types/classes_and_hkts.md) for the surface features summarized here
