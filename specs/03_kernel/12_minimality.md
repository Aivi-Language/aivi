# Minimality proof (informal)

| Feature | Kernel primitive |
| :--- | :--- |
| Lambdas | λ |
| Multi-arg functions | currying |
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
