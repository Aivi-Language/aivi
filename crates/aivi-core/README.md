# aivi-core

## Purpose

First fully-typed intermediate representation (IR) after HIR lowering.
`aivi-core` owns typed expression nodes, signal graph, source lifecycle nodes, decode-program
nodes, and normalized pipe-stage plans (gates, truthy/falsy, fanout, recurrence, temporal).
It sits between `aivi-hir` (frontend) and `aivi-lambda` (closure lowering) in the pipeline.
The current slice is intentionally narrow: it only accepts elaboration reports that `aivi-hir`
can already justify, and rejects blocked handoffs explicitly.

## Entry points

```rust
// Lower a HIR module into a typed core Module
lower_module(hir: &hir::Module, db: &SourceDatabase, reports: &ElaborationReports) -> Result<Module, LoweringErrors>
lower_module_with_items(hir: &hir::Module, items: &IncludedItems, ...) -> Result<Module, LoweringErrors>

// Lower only the runtime fragment (for interpreter/GTK paths)
lower_runtime_module(hir: &hir::Module, ...) -> Result<LoweredRuntimeFragment, LoweringErrors>
lower_runtime_module_with_items(hir: &hir::Module, items: &IncludedItems, ...) -> Result<LoweredRuntimeFragment, LoweringErrors>
lower_runtime_fragment(spec: RuntimeFragmentSpec, ...) -> Result<LoweredRuntimeFragment, LoweringErrors>

// Validate a lowered module
validate_module(module: &Module) -> Result<(), ValidationErrors>
```

## Invariants

- All typed IDs (`ExprId`, `ItemId`, `SourceId`, `StageId`, …) are scoped to the owning `Module`; cross-module ID use is undefined.
- `lower_module` never panics; lowering failures are returned as `LoweringErrors`.
- Every `Expr` node is typed; a missing type annotation is a lowering error, not a silent hole.
- `Module` arenas are append-only after construction; structural mutation is not supported.
- `validate_module` is a post-condition check — passing validation means the IR is internally consistent.
- `IncludedItems` is a `HashSet<hir::ItemId>`; only reachable items are lowered into the module.

## Diagnostic codes

This crate does not emit `DiagnosticCode` values — lowering errors are returned as structured
`LoweringError` / `ValidationError` types. Callers map these to diagnostics as needed.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.3 (typed core IR).
