# Minimality proof (informal)

| Feature | Kernel primitive |
| :--- | :--- |
| Lambdas | λ |
| Multi-arg functions | currying |
| Recursion | `let rec` |
| Patterns | case |
| Records | closed records + update |
| Patching | update + fold |
| Predicates | λ + case |
| Generators | fold (Church-encoded) |
| Effects (`do Effect {}`) | `bind` + `pure` (nested λ / application) |
| Resources | `__makeResource` + `__withResourceScope` (effects + finalizer) |
| Plain blocks | immediately-applied λ |
| Domains | static rewrite |
| Sigils | function call |
| Modules | namespaces |
| Classes | dictionaries (records) |
| HKTs | ∀ |

Nothing else is required.


## The true kernel

> **AIVI's kernel is simply:**
> **λ-calculus with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.**
> **All block forms — `do Effect {}`, `resource {}`, `generate {}`, and `plain {}` — are fully desugared into nested λ/application/`bind`/`pure` at the kernel level.**
> **Domains are static rewrite rules; patching, predicates, generators, resources, and effects are all elaborations of these primitives.**

## Block desugaring

All four block kinds are eliminated during HIR→Kernel lowering:

| Block kind | Desugared form |
| :--- | :--- |
| `do Effect { x <- e1; e2 }` | `__withResourceScope(bind e1 (λx → e2))` |
| `do Effect { x = e; rest }` | `bind (pure e) (λx → rest)` — let-binds are `pure`-wrapped at HIR |
| `resource { acq; yield v; cleanup }` | `__makeResource (λ_ → acq_chain) (λ_ → cleanup_chain)` |
| `generate { yield e; ... }` | Church-encoded fold (existing) |
| `plain { x = e; body }` | `(λx → body) e` — immediately-applied lambda |

The `KernelExpr` enum contains no `Block`, `BlockKind`, or `BlockItem` variants.
