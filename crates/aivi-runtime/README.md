# aivi-runtime

## Purpose

Reactive scheduler, signal propagation, source providers, task execution, and GLib integration.
`aivi-runtime` is the execution engine that drives AIVI programs at run time. It owns the
`Scheduler`, `SignalGraph`, source provider catalog, and GLib main-context adapter. It bridges
backend-compiled programs (`aivi-backend`) and the GTK bridge (`aivi-gtk`) through a shared
reactive layer.

## Entry points

```rust
// Build a signal graph
SignalGraphBuilder::new() -> SignalGraphBuilder
SignalGraphBuilder::add_input(spec: InputSpec) -> InputHandle
SignalGraphBuilder::add_derived(spec: DerivedSpec) -> DerivedHandle
SignalGraphBuilder::add_signal(spec: SignalSpec) -> SignalHandle
SignalGraphBuilder::build() -> Result<SignalGraph, GraphBuildError>

// Reactive scheduler
Scheduler::new() -> Scheduler
Scheduler::publish(handle: InputHandle, value: RuntimeValue) -> Result<(), SchedulerAccessError>
Scheduler::tick() -> TickOutcome

// GLib main-context integration
GlibSchedulerDriver::new(scheduler: Scheduler) -> GlibSchedulerDriver
GlibSchedulerDriver::attach(ctx: &glib::MainContext)
GlibLinkedRuntimeDriver::new(...) -> GlibLinkedRuntimeDriver

// HIR runtime assembly (interpreter path)
assemble_hir_runtime(module: &hir::Module, ...) -> Result<HirRuntimeAssembly, HirRuntimeAdapterErrors>
assemble_hir_runtime_with_items(module: &hir::Module, items: &IncludedItems, ...) -> Result<HirRuntimeAssembly, HirRuntimeAdapterErrors>

// Backend runtime linking (compiled path)
link_backend_runtime(program: &backend::Program, ...) -> Result<BackendLinkedRuntime, BackendRuntimeLinkErrors>

// Task execution
execute_runtime_task_plan(plan: &RuntimeTaskPlan, ctx: &mut dyn ...) -> Result<RuntimeValue, RuntimeTaskExecutionError>
decode_external(program: &SourceDecodeProgram, value: &ExternalSourceValue) -> Result<RuntimeValue, SourceDecodeError>
```

## Invariants

- **Scheduler thread**: the `Scheduler` owns its tick loop; `publish` is the only method safe to call from worker threads.
- **Worker isolation**: workers never hold direct references to scheduler-owned state; they send immutable `Publication` messages via `WorkerPublicationSender`.
- **Signal propagation**: each `Scheduler::tick` is a single atomic batch — signals are propagated in topological order, glitch-free, with no stale reads within a tick.
- **GLib main thread**: `GlibSchedulerDriver::attach` must be called on the GLib main thread; the driver then drives ticks from GLib idle callbacks.
- `SignalGraph` is immutable after `build()`; adding nodes after build is not supported.
- Source providers run on worker threads; they publish to `SourcePublicationPort` and never call scheduler methods directly.
- `decode_external` is pure and side-effect-free; it may be called from any thread.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Runtime errors are returned as structured typed
variants (`SchedulerAccessError`, `GraphBuildError`, `HirRuntimeAdapterErrors`, etc.).

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.3 (reactive runtime and scheduler).
