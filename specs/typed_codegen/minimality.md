# Minimality proof (informal)

| Feature                  | Kernel primitive                                  |
|:------------------------ |:------------------------------------------------- |
| Lambdas                  | λ                                                 |
| Multi-arg functions      | currying                                          |
| Recursion                | `let rec`                                         |
| Patterns                 | case                                              |
| Records                  | closed records + update                           |
| Patching                 | update + fold                                     |
| Predicates               | λ + case                                          |
| Generators               | fold (Church-encoded)                             |
| Effects (`do Effect {}`) | `bind` + `pure` (nested λ / application)          |
| Generic do-monads        | `chain` + `of` (desugared at HIR level)           |
| Resources                | `__makeResource` (λ-wrapped acquire/cleanup pair) |
| Plain blocks             | immediately-applied λ                             |
| Domains                  | static rewrite                                    |
| Sigils                   | function call                                     |
| Modules                  | namespaces                                        |
| Classes                  | dictionaries (records)                            |
| HKTs                     | ∀                                                 |

Nothing else is required.

## The true kernel

> **AIVI's kernel is simply:**
> **λ-calculus with algebraic data types, closed records with update, universal types, fold, and an opaque effect monad.**
> **All block forms — `do Effect {}`, `do Option {}`, `do Result {}`, `resource {}`, `generate {}`, and `plain {}` — are fully desugared into nested λ/application/`bind`/`pure`/`chain`/`of` before reaching the Kernel.**
> **Domains are static rewrite rules; patching, predicates, generators, resources, and effects are all elaborations of these primitives.**

## Block desugaring

Generic do-monads (`do Option`, `do Result`, `do List`) are desugared at HIR lowering
into `chain`/`of` calls. Effect blocks and structural blocks are eliminated during
HIR→Kernel lowering. After Kernel, no `Block`, `BlockKind`, or `BlockItem` variants
remain.

| Block kind                           | Stage         | Desugared form                                                    |
|:------------------------------------ |:------------- |:----------------------------------------------------------------- |
| `do Option { x <- e1; e2 }`          | Surface → HIR | `chain e1 (λx → e2)` — generic monadic desugaring                 |
| `do Result { x <- e1; e2 }`          | Surface → HIR | `chain e1 (λx → e2)` — generic monadic desugaring                 |
| `do Effect { x <- e1; e2 }`          | HIR → Kernel  | `__withResourceScope(bind e1 (λx → e2))`                          |
| `do Effect { x = e; rest }`          | HIR → Kernel  | `bind (pure e) (λx → rest)` — let-binds are `pure`-wrapped at HIR |
| `resource { acq; yield v; cleanup }` | HIR → Kernel  | `__makeResource (λ_ → acq_chain) (λ_ → cleanup_chain)`            |
| `generate { yield e; ... }`          | HIR → Kernel  | Church-encoded fold via `gen_bind`/`gen_yield`/`gen_append`       |
| `plain { x = e; body }`              | HIR → Kernel  | `(λx → body) e` — immediately-applied lambda                      |

The `KernelExpr` enum contains no `Block`, `BlockKind`, or `BlockItem` variants.

## Optimization variants

Several `KernelExpr` variants are theoretically reducible to the core λ-calculus +
case primitives, but are preserved through to the Cranelift backend for code quality:

| Variant           | Theoretical reduction                | Why it is kept                                                        |
|:----------------- |:------------------------------------ |:--------------------------------------------------------------------- |
| `If`              | `Match` on `Bool` (`True`/`False`)   | Cranelift emits a single `brif` branch vs. full match compilation     |
| `Binary`          | `App(App(Var(op), left), right)`     | Cranelift uses native int/float machine instructions when types known |
| `Call`            | Nested `App` (curried application)   | Avoids intermediate closure allocation for multi-arg calls            |
| `Pipe`            | `App(func, arg)`                     | Carries debug metadata (`pipe_id`, `step`, `label`) for `@debug`      |
| `TextInterpolate` | Chain of `++` (string concatenation) | Cranelift builds interpolated strings in a single pass                |

These variants do not add expressive power — every program using them can be
rewritten using only λ, application, and case. They exist because the Cranelift
backend produces significantly better native code for these common patterns.

## Dead `RustIrExpr::Block` code

<!-- TODO: cleanup candidate -->

`RustIrExpr` re-defines `Block`, `RustIrBlockKind`, and `RustIrBlockItem` variants
that mirror the HIR block types. However, the Kernel pass has **already eliminated
all blocks** before the Kernel→RustIr lowering runs, so `lower_kernel()` never
produces `RustIrExpr::Block`.

Despite this, the Cranelift backend contains full native block compilation
infrastructure:

| File              | Dead block code                                                               |
|:----------------- |:----------------------------------------------------------------------------- |
| `lower.rs`        | `lower_block`, `lower_do_block`, `lower_native_generate`, `lower_plain_block` |
| `inline.rs`       | Cost computation, reference detection, substitution for Block nodes           |
| `use_analysis.rs` | Usage tracking and traversal for Block nodes                                  |
| `compile.rs`      | `is_effect_block` detection, `expr_supported`, `collect_called_globals`       |

This code is structurally correct but unreachable.  It should either be:

- **Removed** as dead code (simplest), or
- **Activated** by forwarding blocks from HIR directly to RustIr (skipping Kernel
  desugaring), which would let Cranelift compile effect blocks and generators
  natively instead of going through Church-encoded/monadic intermediate forms.

## Structural duplication across IRs

The three IR stages (HIR, Kernel, RustIr) share nearly identical type definitions
for non-expression types:

| Type             | HIR                   | Kernel                   | RustIr                   | Identical? |
|:---------------- |:--------------------- |:------------------------ |:------------------------ |:---------- |
| Pattern          | `HirPattern` (8)      | `KernelPattern` (8)      | `RustIrPattern` (8)      | Yes        |
| Literal          | `HirLiteral` (5)      | `KernelLiteral` (5)      | (inline in Expr)         | Yes        |
| ListItem         | `HirListItem`         | `KernelListItem`         | `RustIrListItem`         | Yes        |
| RecordField      | `HirRecordField`      | `KernelRecordField`      | `RustIrRecordField`      | Yes        |
| MatchArm         | `HirMatchArm`         | `KernelMatchArm`         | `RustIrMatchArm`         | Yes        |
| TextPart         | `HirTextPart`         | `KernelTextPart`         | `RustIrTextPart`         | Yes        |
| MockSubstitution | `HirMockSubstitution` | `KernelMockSubstitution` | `RustIrMockSubstitution` | Yes        |

The only real differences between IRs are:

1. **HIR → Kernel**: `Block` variant eliminated (the one genuine transform)
2. **Kernel → RustIr**: `Var` split into `Local`/`Global`/`Builtin`/`ConstructorValue`;
   `PathSegment::All` expanded to `IndexFieldBool`/`IndexPredicate`

The 22 expression variants that map 1:1 from HIR → Kernel → RustIr account for
~1,500 lines of mechanical copy code across `kernel/ir.rs` and `rust_ir/lowering.rs`.
