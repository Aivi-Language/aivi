# aivi-lambda

## Purpose

Closed typed-lambda IR — the layer between `aivi-core` and `aivi-backend` that makes closure
boundaries explicit. `aivi-lambda` re-expresses core items as module-owned `Closure` nodes with
analyzed captured environments, stable `CaptureId` / `ClosureId` identities, and explicit
`ClosureKind` (ordinary vs. runtime-body). It does not yet commit to backend ABI or layout;
those decisions belong to `aivi-backend`.

## Entry points

```rust
// Lower a typed-core Module into a lambda Module
lower_module(core: &core::Module) -> Result<Module, LoweringErrors>

// Validate the lowered lambda Module
validate_module(module: &Module) -> Result<(), ValidationErrors>
```

Key types exposed for downstream consumers:

```rust
Closure      // A single closure node with kind, parameters, captures, and body expr
Capture      // A captured variable: source binding + CaptureId
ClosureKind  // Ordinary | RuntimeBody
CaptureId    // Stable identity for a capture slot within a closure
ClosureId    // Stable identity for a closure within the module
```

Most typed-core types (`Expr`, `ExprId`, `Type`, `SignalInfo`, `SourceNode`, `DecodeProgram`, …)
are re-exported directly from `aivi-core` so callers need not depend on both crates.

## Invariants

- Every `Closure` has exactly one `ClosureKind`; the kind is immutable after construction.
- `CaptureId` and `ClosureId` are scoped to the owning `Module`; cross-module use is undefined.
- `lower_module` is total for valid core modules; errors indicate an internal invariant violation.
- Captured environments are analyzed once during lowering; the result is carried as stable lambda metadata.
- `validate_module` checks that every `Capture` references a valid binding and every `ClosureId` resolves within the module.
- This crate re-exports core types verbatim; there is no additional type elaboration at this layer.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Lowering and validation errors are returned as
structured `LoweringErrors` / `ValidationErrors` types.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.4 (lambda IR and closure lowering).
