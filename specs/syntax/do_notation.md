# Generic Monadic `do` Blocks

> **Status**: Implemented (v0.1)   `do M { ... }` works for any type constructor with a `Chain` instance (including `Option`, `Result`, and `List`). Blocks are desugared to nested `chain`/lambda calls during HIR lowering. Native codegen is stubbed.  
> **Depends on**: Type classes ([§ 3.5](03_types.md#35-classes-and-hkts)), `Monad` hierarchy ([aivi.logic](../05_stdlib/00_core/03_logic.md)), effects ([§ 9](09_effects.md)), instance resolution (compiler).

## Overview

`do M { ... }` generalizes the existing `do Effect { ... }` block to work with **any type constructor `M` that has a `Monad` instance**. The `<-` and `=` syntax remains identical; the compiler resolves `chain`/`of` from the `Monad` dictionary for `M` instead of hardcoding `Effect` primitives.

### Design Principles

1. **`do Effect { ... }` remains the primary form**   it is the most common and retains its special features (`or` fallback, `resource` acquisition, `when`/`unless`/`given`, `loop`/`recurse`, `on`).
2. **`generate { ... }` stays separate**   generators have fundamentally different semantics (`yield`, guards, pull-based), and are not monadic in the standard sense.
3. **Generic `do M` supports only the common monadic subset**   `<-` (bind), `=` (let), final expression. Effect-specific statements are not available in generic blocks.
4. **Instance-driven**   the compiler uses the existing class/instance resolution to find `Chain M` (for `chain`) and `Applicative M` (for `of`).

## Syntax

### Grammar extension

The existing grammar rule:

```text
DoBlock := "do" UpperIdent "{" { DoStmt } "}"
```

is **unchanged**   the parser already accepts any `UpperIdent` after `do`. The change is semantic: the type checker and desugaring must handle the monad name generically.

### Statement subset by monad

| Statement | `do Effect` | `do M` (generic) | `generate` |
| :--- | :---: | :---: | :---: |
| `x <- expr` | yes | yes | yes (from sequence) |
| `x = expr` | yes | yes | yes |
| `expr` (sequencing) | yes | yes | no |
| `yield expr` | no | no | yes |
| `x -> pred` (guard) | no | no | yes |
| `or` fallback | yes | **no** | no |
| `when cond <- eff` | yes | **no** | no |
| `unless cond <- eff` | yes | **no** | no |
| `given cond or expr` | yes | **no** | no |
| `on Event => handler` | yes | **no** | no |
| `loop`/`recurse` | yes | **no** (v1) | yes |
| resource `<-` | yes | **no** | no |

Rationale: `or`, `when`/`unless`/`given`, `on`, and resource acquisition are tightly coupled to the `Effect E A` type (error handling, cancellation, cleanup). Generic monadic blocks use only the universal monadic operations.

## Desugaring

### Generic `do M { ... }`

A `do M { ... }` block desugars to calls to `chain` and `of` from the `Chain M` and `Applicative M` dictionaries.

#### Bind

<<< ../snippets/from_md/02_syntax/16_do_notation/block_01.aivi{aivi}


desugars to:

```text
chain (λx. ⟦do M { body }⟧) ⟦expr⟧
```

(using `chain : (A -> M B) -> M A -> M B` from `Chain M`)

#### Pure let-binding

<<< ../snippets/from_md/02_syntax/16_do_notation/block_02.aivi{aivi}


desugars to:

```text
let x = ⟦expr⟧ in ⟦do M { body }⟧
```

#### Sequencing (expression statement)

<<< ../snippets/from_md/02_syntax/16_do_notation/block_03.aivi{aivi}


desugars to:

```text
chain (λ_. ⟦do M { body }⟧) ⟦expr⟧
```

#### Final expression

<<< ../snippets/from_md/02_syntax/16_do_notation/block_04.aivi{aivi}


desugars to `⟦expr⟧`. It must have type `M A`.

#### Empty block

<<< ../snippets/from_md/02_syntax/16_do_notation/block_05.aivi{aivi}


desugars to `of Unit` (using `of : A -> M A` from `Applicative M`).

### `do Effect { ... }` as a specialization

`do Effect { ... }` becomes sugar for `do (Effect E) { ... }` **plus** the effect-specific extensions (fallback, guards, resources, etc). In terms of desugaring:

- `chain` for `Effect E` is `bind : Effect E A -> (A -> Effect E B) -> Effect E B`
- `of` for `Effect E` is `pure : A -> Effect E A`
- The additional statements (`or`, `when`, `given`, `on`, `loop`, resource `<-`) are desugared as specified in [Effects § 9](09_effects.md) and [Desugaring § 7](../04_desugaring/07_effects.md).

The compiler detects `do Effect` specifically (by name) to enable the extended statement set. All other `do M` blocks get the generic subset only.

## Type Checking

### Current state (v0.1)

The parser stores `BlockKind::Do { monad: SpannedName }` and already accepts any identifier. However:
- HIR lowering discards the monad name (`Do { .. }` → `HirBlockKind::Effect`)
- The type checker hardcodes `Type::con("Effect")`

### Required changes

1. **Preserve monad identity through HIR/kernel**: `HirBlockKind::Do { monad: SpannedName }` instead of just `HirBlockKind::Effect`. Keep `Effect` as a recognized special case.

2. **Resolve Monad instance**: When the monad is not `Effect`, the type checker must:
   - Resolve the type constructor `M` from the identifier
   - Find a `Monad M` instance (which implies `Chain M` + `Applicative M`)
   - Extract `chain : (A -> M B) -> M A -> M B` and `of : A -> M A`
   - Unify `<-` RHS with `M A` and bind `x : A`
   - Unify the block's overall type with `M R` (where `R` is the final expression's inner type)

3. **Reject effect-specific statements**: If `M ≠ Effect`, emit errors for `or`, `when`, `unless`, `given`, `on`, resource binds.

4. **Error messages**: When no `Monad` instance exists for `M`:
   ```
   error[E????]: `do` block requires a Monad instance
     --> file.aivi:5:1
     |
   5 | do MyType {
     |    ^^^^^^ no `Monad (MyType *)` instance found
     |
     = help: define `instance Monad (MyType *) = { ... }`
   ```

### Runtime

For the interpreted runtime, generic `do M` blocks must:
- Look up the `chain` and `of` methods from the `Monad` dictionary
- Apply them to evaluate `<-` binds and sequencing

This contrasts with `do Effect`, which uses the built-in `Effect` runtime machinery.

## Monadic Types That Benefit

### Already defined in AIVI

| Type | Monad instance | `do` block example | Use case |
| :--- | :--- | :--- | :--- |
| `Option A` | `Monad (Option *)` | `do Option { x <- lookup k m; pure (x + 1) }` | Short-circuit chaining when any step returns `None` |
| `Result E A` | `Monad (Result E *)` | `do Result { x <- parse input; validate x }` | Pure error chaining without effects |
| `List A` | needs instance | `do List { x <- [1,2,3]; y <- [4,5]; pure (x,y) }` | Non-deterministic computation / cartesian products |

### Potential future additions

| Type | Description | `do` block benefit |
| :--- | :--- | :--- |
| `Validation E A` | Like `Result` but **accumulates** errors (via `Applicative`, not short-circuiting `chain`). | `do Validation { name <- validateName input; age <- validateAge input; pure { name, age } }`   note: `Validation` only benefits from `ap` (applicative), not `chain`. A `do` block would be misleading since `<-` suggests sequencing. Better served by applicative combinators. |
| `Reader R A` | Computation with read-only environment. | `do Reader { env <- ask; pure (env.dbUrl) }`   useful for dependency injection patterns. |
| `Writer W A` | Computation that accumulates a log (`W : Monoid`). | `do Writer { tell "step 1"; x <- compute; tell "step 2"; pure x }`   structured logging. |
| `State S A` | Computation with read-write state. | `do State { s <- get; put (s + 1); pure s }`   stateful algorithms without effects. |
| `Parser A` | Parser combinators. | `do Parser { x <- literal "if"; _ <- spaces; cond <- expr; pure (If cond) }`   parser composition. |

### Recommendation for v0.2

Start with **`Option`** and **`Result`** only   they already have `Monad` instances in the spec and address the most common pain points. `List` should get an instance too but its use via `do` overlaps heavily with `generate { ... }`.

`Validation`, `Reader`, `Writer`, `State`, and `Parser` should be evaluated individually as stdlib additions; they are not required for the `do M` mechanism itself.

## Implementation Plan

### Phase 1: Plumbing (preserve monad identity)

**Scope**: No new user-visible behavior yet   just stop discarding information.

| Layer | Change |
| :--- | :--- |
| HIR | Add `HirBlockKind::Do { monad: String }` (keep `Effect` as a recognized name) |
| HIR lowering | Preserve monad name from surface AST instead of mapping all `Do { .. }` to `Effect` |
| Kernel IR | Add `KernelBlockKind::Do { monad: String }` similarly |
| Kernel lowering | Preserve monad name |
| Type checker | When `monad == "Effect"`, use existing hardcoded logic (no behavior change) |
| Runtime | When block kind is `Do { monad: "Effect" }`, use existing effect machinery |

**Validation**: All existing tests pass. `do Effect { ... }` works identically.

### Phase 2: Reject effect-specific statements for non-Effect monads

**Scope**: `do Option { ... }` parses and type-checks, but only the common subset is allowed.

| Step | Detail |
| :--- | :--- |
| Parser validation | When `monad != "Effect"`, emit errors for `or`, `when`, `unless`, `given`, `on`, resource `<-` |
| Error messages | Clear diagnostics: "statement X is only available in `do Effect` blocks" |

### Phase 3: Generic type checking

**Scope**: `do M { ... }` type-checks against `Monad M`.

| Step | Detail |
| :--- | :--- |
| Instance lookup | In `infer_do_block`, when `monad != "Effect"`, resolve `Chain M` and `Applicative M` instances |
| Bind typing | Unify `<-` RHS with `M A`, bind variable as `A` |
| Block return type | The block has type `M R` where `R` is the final expression's unwrapped type |
| `of` insertion | Empty block or implicit `pure Unit` → call `of Unit` from the `Applicative` dictionary |

### Phase 4: Generic runtime / codegen

**Scope**: `do M { ... }` actually runs.

| Step | Detail |
| :--- | :--- |
| Desugaring pass | Transform `do M { x <- e; body }` into `chain (λx. body) e` at HIR or kernel level |
| Runtime dispatch | Instead of special-casing `Effect`, emit dictionary-passing calls to `chain`/`of` |
| Or: keep interpreted | Alternative: the runtime can interpret `Do { monad }` blocks by looking up the dictionary at runtime. Less efficient but simpler initially. |

### Phase 5: Integration tests and stdlib

| Step | Detail |
| :--- | :--- |
| `do Option` tests | Short-circuit None propagation |
| `do Result` tests | Pure error chaining |
| `do List` tests (if instance added) | Cartesian product / non-determinism |
| Spec updates | Update [§ 9.8](09_effects.md#98-do-notation-scope-v01) to remove the v0.1 restriction |
| AIVI_LANGUAGE.md | Already mentions `do M { ... }`   verify consistency |

## Open Questions

1. **Should `do M { }` (empty) desugar to `of Unit` or be an error?**
   Proposal: `of Unit`   consistent with `do Effect { }` → `pure Unit`.

2. **Should `given`/`when`/`unless` be generalizable?**
   `given cond or expr` could work for any `M` that supports short-circuiting (e.g. `Option`, `Result`). However, the semantics differ per monad (Effect: `fail`, Option: `None`, Result: `Err ...`). Keep Effect-only for now; revisit with a `MonadFail` class if needed.

3. **`loop`/`recurse` in generic `do`?**
   `loop` in `do Effect` desugars to a local recursive function whose body is an effect block. The same pattern works for any monad. Could be enabled by desugaring `loop` to a `let rec` + `chain` pattern. Defer to a later version to keep scope small.

4. **Should `do (Effect E)` be valid syntax (explicit type application)?**
   The grammar says `"do" UpperIdent`, so `do (Effect E)` would require a grammar change to `"do" TypeExpr`. Not needed for v0.2   the compiler infers `E` from context.

5. **Higher-kinded application: `do (Result E) { ... }`?**
   `Result E` is a partially applied type constructor `Result E *`. The grammar would need to accept type expressions, not just identifiers. Defer   `do Result { ... }` can work if the compiler resolves `Result` as `Result E *` with `E` inferred.

6. **Name resolution: `do MyModule.MyMonad { ... }`?**
   Qualified names after `do` would require extending the grammar from `UpperIdent` to `QualifiedUpperIdent`. Low priority.

7. **Interaction with `Validation` (Applicative-only)?**
   `Validation` accumulates errors via `ap` but its `chain` is sequential (same as `Result`). A `do Validation` block would chain sequentially, not accumulate. This is a known monad/applicative tension. Options:
   - Don't provide `Monad` for `Validation` (force applicative style)
   - Accept the sequential behavior with documentation
   - Introduce `ado` (applicative-do) as a separate feature

## References

- Effects: [§ 9](09_effects.md)
- Generators: [§ 7](07_generators.md)
- Type classes: [§ 3.5](03_types.md#35-classes-and-hkts)
- Monad hierarchy: [aivi.logic](../05_stdlib/00_core/03_logic.md)
- Desugaring   effects: [§ 7](../04_desugaring/07_effects.md)
- Desugaring   classes: [§ 8](../04_desugaring/08_classes.md)
- Current v0.1 restriction: [§ 9.8](09_effects.md#98-do-notation-scope-v01)
