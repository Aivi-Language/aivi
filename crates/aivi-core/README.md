# aivi-core

## Purpose

First fully-typed intermediate representation (IR) after HIR lowering.
`aivi-core` owns typed expression nodes, signal graph, source lifecycle nodes, decode-program
nodes, and normalized pipe-stage plans (gates, truthy/falsy, fanout, recurrence, temporal).
It sits between `aivi-hir` (frontend) and `aivi-lambda` (closure lowering) in the pipeline.
The current slice is intentionally narrow: it only accepts elaboration reports that `aivi-hir`
can already justify, and rejects blocked handoffs explicitly.

## Entry points

- [`lower_module`](src/lower/api.rs) accepts a HIR module and returns `Result<Module, LoweringErrors>`.
- `lower_module_with_items` restricts lowering to an `IncludedItems` set.
- `lower_runtime_module` and its item/workspace variants lower the supported runtime surface.
- `lower_runtime_fragment` and its workspace variant return a `LoweredRuntimeFragment`.
- [`validate_module`](src/validate.rs) checks a lowered core module.

Lowering obtains its elaboration evidence internally; callers do not pass a source database or an `ElaborationReports` argument.

## Invariants

- All typed IDs (`ExprId`, `ItemId`, `SourceId`, `StageId`, …) are scoped to the owning `Module`; cross-module ID use is invalid.
- Unsupported lowering is reported as `LoweringErrors`; internal arena/identity preconditions still apply.
- Every `Expr` node is typed; a missing type annotation is a lowering error, not a silent hole.
- `Module` arenas are append-only after construction; structural mutation is not supported.
- `validate_module` is a post-condition check — passing validation means the IR is internally consistent.
- `IncludedItems` is a `HashSet<hir::ItemId>`; only reachable items are lowered into the module.

## Diagnostic codes

This crate does not emit `DiagnosticCode` values — lowering errors are returned as structured
`LoweringError` / `ValidationError` types. Callers map these to diagnostics as needed.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.3 (typed core IR).
