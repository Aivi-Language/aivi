# Equality Semantics

AIVI v1 has **compiler-derived structural `Eq`** for many concrete closed types. This is broader than some user-facing docs imply.

## Concrete closed types

**Sources**: `crates/aivi-hir/src/typecheck/checker.rs`, `crates/aivi-typing/src/eq.rs`

For direct `==` / `!=` on a concrete value, the type checker accepts:

- primitive types except `Bytes`
- tuples whose elements have `Eq`
- closed records whose fields have `Eq`
- closed sum types whose payloads have `Eq`
- domains whose carrier has `Eq`
- `List`, `Option`, `Result`, `Validation` when contained types have `Eq`

This is why code like `Coord 1 2 == Coord 1 2` checks successfully for a user-defined closed sum such as:

```aivi
type Coord = Coord Int Int
```

The implementation lives in `require_eq_with_scope()` and `require_compiler_derived_eq_with_scope()` in `crates/aivi-hir/src/typecheck/checker.rs:3340-3495`. The lower-level structural deriver in `crates/aivi-typing/src/eq.rs:523-731` mirrors this model and explicitly rejects open records, open sums, recursive derivations, and `Bytes`.

## Why demos still define `coordEq` / `cellEq`

**Sources**: `demos/reversi.aivi`, `demos/snake.aivi`, `stdlib/aivi/list.aivi`, `stdlib/aivi/prelude.aivi`

Some stdlib helpers are comparator-passing APIs rather than class-constrained APIs. Example:

- `stdlib/aivi/list.aivi:401-403` defines `contains : (A -> A -> Bool) -> A -> List A -> Bool`
- `stdlib/aivi/prelude.aivi:249-251` re-exports that shape as ambient `contains`

So demos wrap `==` into a named comparator:

```aivi
type Cell -> Cell -> Bool
func cellEq = a b =>
    a == b
```

That helper exists so code can **pass equality as first-class function value** into `listContains` / `contains`. It is not proof that `Cell` or `Coord` lack structural equality.

In a few places, demos also call `coordEq cell target` where plain `cell == target` would also work. That is best read as style or reuse, not a language restriction.

## Where `Eq` still needs help

**Sources**: `manual/guide/classes.md`, `crates/aivi-hir/src/typecheck/checker.rs`

For **open type parameters**, `==` still needs explicit evidence:

```aivi
type Eq K => K -> K -> Bool
func matchesKey = key candidate =>
    key == candidate
```

Without that constraint, the checker reports that the open type parameter has no compiler-derived `Eq` instance. This is separate from concrete-type structural equality.

## Current exclusions

`Eq` is not compiler-derived in v1 for:

- `Bytes`
- functions / arrows
- `Signal`
- `Task`
- `Map`
- `Set`

Imported opaque types are accepted optimistically at use sites; their defining module is expected to validate equality there.
