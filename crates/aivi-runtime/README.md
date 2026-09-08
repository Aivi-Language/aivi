# aivi-runtime

## Purpose

Reactive scheduler, signal propagation, source providers, task execution, and GLib integration.
`aivi-runtime` is the execution engine that drives AIVI programs at run time. It owns the
`Scheduler`, `SignalGraph`, source provider catalog, and GLib main-context adapter. It bridges
backend-compiled programs (`aivi-backend`) and the GTK bridge (`aivi-gtk`) through a shared
reactive layer.

## Entry points

- [`SignalGraphBuilder`](src/graph.rs) constructs and validates the static graph.
- [`Scheduler::new`](src/scheduler.rs) takes that graph; `tick` takes a mutable evaluator.
- `Scheduler::worker_sender` gives workers a `WorkerPublicationSender`; workers publish `Publication` messages through that sender.
- [`GlibSchedulerDriver::new`](src/glib_adapter.rs) takes a `MainContext`, scheduler, and evaluator. Its worker sender also requests a main-context wakeup.
- `GlibLinkedRuntimeDriver` coordinates a linked runtime on a GLib context.
- [Crate exports](src/lib.rs) expose HIR assembly, backend linking, task execution, and external decoding.

There is no `Scheduler::publish` or `GlibSchedulerDriver::attach` API. Keep the scheduler and its evaluator on their owning execution path.

## Invariants

- **Scheduler thread**: the owner drives `Scheduler::tick`; workers use publication senders, not direct scheduler mutation.
- **Worker isolation**: workers never hold direct references to scheduler-owned state; they send immutable `Publication` messages via `WorkerPublicationSender`.
- **Signal propagation**: each `Scheduler::tick` is a single atomic batch — signals are propagated in topological order, glitch-free, with no stale reads within a tick.
- **GLib main thread**: GLib drivers execute ticks on their owned `MainContext`; worker senders enqueue messages and request wakeups.
- `SignalGraph` is immutable after `build()`; adding nodes after build is not supported.
- Blocking provider work runs on workers and publishes through `SourcePublicationPort`; GTK/GDK input integration stays on the UI context.
- `decode_external` is pure and side-effect-free; it may be called from any thread.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Runtime errors are returned as structured typed
variants (`SchedulerAccessError`, `GraphBuildError`, `HirRuntimeAdapterErrors`, etc.).

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.3 (reactive runtime and scheduler).
