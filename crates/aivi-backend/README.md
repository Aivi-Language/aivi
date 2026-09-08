# aivi-backend

## Purpose

Backend IR, layout decisions, and Cranelift codegen/runtime execution.
`aivi-backend` consumes the validated `aivi-lambda` module and re-expresses it as layout-aware,
ABI-explicit backend structures: `Program`, `Pipeline`, `Kernel`, `SourcePlan`, `DecodePlan`,
and `Layout` tables. It lowers backend-owned kernels through Cranelift into object bytes for AOT
surfaces and into a lazy JIT engine for live `aivi run` / runtime-fragment execution. The full
`aivi build` bundle (linking, runtime embedding) is performed by `aivi-cli`; this crate owns the
backend-side machine-code caches and runtime execution engine.

## Entry points

- [`lower_module`](src/lower.rs) lowers a lambda module to `Result<Program, LoweringErrors>`.
- [`validate_program`](src/validate.rs) checks backend invariants.
- [`compile_program`](src/codegen/errors_api.rs) emits `Result<CompiledProgram, CodegenErrors>`.

Key runtime types used by the live execution path:

```rust
RuntimeValue               // Dynamically-typed runtime value (GTK/runtime path)
BackendExecutableProgram   // Builds the active lazy-JIT execution surface
KernelEvaluator            // Reference interpreter and fallback engine
RuntimeTaskPlan             // Describes a task for the host runtime to execute
```

## Invariants

- Unsupported lowering is reported through `LoweringErrors`; valid lambda structure alone does not promise support for every backend layout.
- `Layout` tables are computed once and immutable; all downstream stages read them by `LayoutId`.
- `Kernel` calling conventions are explicit (`CallingConvention`); no implicit ABI inference occurs.
- Cranelift codegen touches only backend-owned kernel bodies; pure core / HIR types are not re-parsed.
- `CompiledProgram` carries object bytes and a symbol table; linking is the caller's responsibility.
- Live execution routes through a lazy per-kernel JIT engine first and falls back to
  `KernelEvaluator` for unsupported layouts/helpers.
- Persistent per-kernel disk artifacts store replayable JIT machine-code bundles keyed by backend
  fingerprint plus compiler/target namespace; corrupt entries degrade to cache misses.
- `RuntimeValue` includes owned `Box` and `Vec` payloads; not all values are inline or reference-counted. `DetachedRuntimeValue` makes owned/copy boundaries explicit, while GC handles serve collector-managed storage.
- This crate has no GTK dependency. Worker use still has to respect the ownership and `Send`/`Sync` requirements of the particular evaluator/JIT value.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Errors are `LoweringErrors`, `ValidationErrors`, or
`CodegenErrors` — structured typed values.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.5 (backend IR and Cranelift codegen).
