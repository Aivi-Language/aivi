# Prelude surface policy

This page fixes the target public shape of `stdlib/aivi/prelude.aivi` for the uniform-elegance cleanup.

## Chosen direction

Prelude should be **class-polymorphic first, carrier-specific second**.

That means:

- canonical generic names and class vocabulary belong in prelude
- carrier-specific helpers stay in their owning modules unless they are truly universal ambient ergonomics
- prelude should teach one algebraic story, not compete with it

## Rules

### 1. Canonical names in prelude must be generic where lawful

When AIVI has a lawful class-backed operation, prelude should prefer that operation and its ordinary surface name.

Examples:

- `map`, not `mapOption` / `mapResult` as primary ambient vocabulary
- `flatMap` / `chain` only where monadic surface is intentional
- `reduce` / `fold` at class or carrier boundary only when naming is semantically clear
- `Eq` / `Ord`-driven collection helpers as canonical forms once available

### 2. Carrier-specific helpers stay module-local by default

Helpers such as:

- `mapOption`
- `mapResult`
- `flatMapOption`
- `flatMapResult`
- `foldOption`
- `foldResult`

may exist, but they should live in `aivi.option`, `aivi.result`, `aivi.validation`, or other owning modules. If prelude re-exports them at all, they should be clearly secondary and not define the main user story.

### 3. Prelude may keep ambient ergonomics only when they are cross-cutting

Good ambient exports:

- core types and constructors
- class names
- operators
- widely useful non-controversial helpers
- indexing helpers already treated as common surface vocabulary

Bad ambient exports:

- parallel families of carrier-specific wrappers that overshadow class-backed forms
- misleading names whose semantics depend on ad hoc conventions

### 4. Validation must be first-class in public surface

If `Validation` is ambient as a type, its main combinators must also be easy to reach. Prelude should not make `Validation` look like a type without a public programming model.

### 5. Compatibility aliases are transitional, not permanent policy

If older concrete-first names remain during migration, they are temporary compatibility surface, not canonical design. Follow-on tasks must either remove them or demote them clearly.

## Consequences for backlog

- `p7` removes or demotes wrapper-heavy ambient exports.
- `p8` makes `Validation` coherent in prelude.
- `p9` and `p10` move list APIs toward lawful `Eq` / `Ord` semantics and away from misleading names.
- `p11` aligns `Matrix` constructors with explicit error policy.

## Review test

Ask of every proposed prelude export:

> If user learns only this name, do they learn AIVI's algebraic model or a local workaround?

Keep export only if answer is algebraic model.
