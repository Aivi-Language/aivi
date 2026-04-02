# aivi-backend

## Purpose

Backend IR, layout decisions, and Cranelift AOT codegen.
`aivi-backend` consumes the validated `aivi-lambda` module and re-expresses it as layout-aware,
ABI-explicit backend structures: `Program`, `Pipeline`, `Kernel`, `SourcePlan`, `DecodePlan`,
and `Layout` tables. It then lowers backend-owned scalar kernels through Cranelift into object
bytes. The full `aivi build` bundle (linking, runtime embedding) is performed by `aivi-cli`; this
crate stops at object emission.

## Entry points

```rust
// Lower a lambda Module into a backend Program
lower_module(lambda: &lambda::Module) -> Result<Program, LoweringErrors>

// Validate a backend Program
validate_program(program: &Program) -> Result<(), ValidationErrors>

// Compile a validated Program to object code
compile_program(program: &Program) -> Result<CompiledProgram, CodegenErrors>
```

Key runtime types used by the interpreter path:

```rust
RuntimeValue         // Dynamically-typed runtime value (interpreter/GTK path)
KernelEvaluator      // Evaluates a backend kernel against RuntimeValues
RuntimeTaskPlan      // Describes an async task to execute
execute_runtime_value(kernel: &Kernel, args: &[RuntimeValue]) -> Result<RuntimeValue, EvaluationError>
```

## Invariants

- `lower_module` is total for valid lambda modules; failures are `LoweringErrors`, never panics.
- `Layout` tables are computed once and immutable; all downstream stages read them by `LayoutId`.
- `Kernel` calling conventions are explicit (`CallingConvention`); no implicit ABI inference occurs.
- Cranelift codegen touches only backend-owned kernel bodies; pure core / HIR types are not re-parsed.
- `CompiledProgram` carries object bytes and a symbol table; linking is the caller's responsibility.
- `RuntimeGcHandle` and `MovingRuntimeValueStore` are the only types that own heap-allocated values at runtime; all other `RuntimeValue` variants are inline or reference-counted.
- GTK main thread: this crate has no GTK dependency and is safe to use from worker threads.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Errors are `LoweringErrors`, `ValidationErrors`, or
`CodegenErrors` — structured typed values.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.5 (backend IR and Cranelift codegen).
