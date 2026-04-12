# Wadler Audit

Focused audit notes for the uniform-elegance cleanup backlog.

## Executable class support documentation

- Canonical human-facing source: `manual/guide/typeclasses.md#canonical-builtin-executable-support`
- Canonical implementation source: `crates/aivi-core/src/class_support.rs`
- Policy: other docs should summarize or link to that section instead of copying support matrices

## Preserved invariants

- `Signal` remains applicative and non-monadic.
- `Validation E` remains applicative and non-monadic.
- `Task E` keeps its current builtin executable `Functor` / `Apply` / `Applicative` / `Chain` / `Monad` support.
- `Traversable` support and traverse-result applicative support stay distinct: `Signal` is allowed only on the result-applicative side, and `Task` is excluded there.
- Docs in this slice should describe `!=` as surface inequality using the same `Eq` evidence as `==`; do not teach separate `(!=)` instance bodies as the canonical user story.
