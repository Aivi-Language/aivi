# Class Laws

Canonical documentation pages:

- `manual/guide/typeclasses.md` — executable support matrix
- `manual/guide/class-laws.md` — user-facing law and rationale page
- `AIVI_RFC.md` §7.4–§7.5 — normative law/guidance summary

## Preserved boundaries

- `Signal` is lawful as `Functor` / `Apply` / `Applicative`, not `Chain` / `Monad`.
- `Validation E` is lawful as an accumulation-oriented `Applicative`, not `Chain` / `Monad`.
- `Task E` keeps builtin executable `Functor` / `Apply` / `Applicative` / `Chain` / `Monad` support.
- `Traversable` carrier support and traverse-result applicative support stay distinct: `Signal` is
  allowed only as a result applicative, and `Task` is still excluded there.

## Law coverage now documented

- `Eq`: reflexive, symmetric, transitive
- `Ord`: total order coherent with `Eq`
- `Functor`: identity, composition
- `Apply`: composition
- `Applicative`: identity, homomorphism, interchange, composition
- `Chain`: associativity
- `Monad`: left identity, right identity, associativity, `join`/`chain` coherence
- `Foldable`: declared-order reduction guidance
- `Traversable`: identity, naturality, composition, shape preservation
- `Filterable`: no reordering/duplication, `filterMap Some = id`
- `Bifunctor`: identity and composition on both sides

## Equality note

Docs now treat `!=` as surface inequality using the same `Eq` evidence as `==`. The raw ambient
prelude source still names both operators internally, so this page records a documentation-level
canonical story rather than a compiler cleanup.
