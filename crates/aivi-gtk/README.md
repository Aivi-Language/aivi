# aivi-gtk

## Purpose

GTK4/libadwaita widget bridge — lowers HIR markup expressions into a typed widget plan, assembles
a runtime adapter, and executes the result through an explicit GTK host boundary.
`aivi-gtk` sits above `aivi-runtime` (for reactive signal plumbing) and below the application
entry point. It does not implement a virtual DOM; widget identity and mutation are explicit.

## Entry points

- [`lower_markup_expr`](src/lower.rs) takes a HIR module and `ExprId`; `lower_markup_root` takes a module and `MarkupNodeId`.
- Workspace/options variants carry explicit lowering context.
- `assemble_widget_runtime` and `WidgetRuntimeAssemblyBuilder` connect widget plans to reactive evaluation.
- [`GtkConcreteHost`](src/host.rs) creates, updates, and disposes concrete host widgets.
- [Schema lookups](src/schema.rs), including `lookup_widget_schema`, `lookup_widget_schema_by_name`, and `supported_widget_schemas`, expose the supported properties, events, and child slots.

Use the source signatures for parameters and ownership; a widget plan is not itself a mounted GTK application.

## Invariants

- **GTK main thread invariant**: all `GtkConcreteHost` operations and any function that creates,
  mutates, or destroys a GTK widget **must** run on the GLib main thread. Do not call GTK from worker threads; GLib contexts and GTK thread affinity are different constraints.
- Widget plans are immutable after construction; runtime updates flow through `WidgetRuntimeAssembly` reactive bindings.
- `lower_markup_expr` never produces partially-lowered plans; on error the full `LoweringError` is returned.
- Property setters and event bindings are statically typed via `GtkPropertyDescriptor` / `GtkEventDescriptor`; unknown properties/events are rejected at plan time, not at mount time.
- Mounted GTK objects and provider subscriptions require explicit unmount/disposal through the host/runtime lifecycle; a host value alone is not a proof that every external reference has been released.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Errors are returned as `LoweringError`,
`WidgetRuntimeAdapterErrors`, and `GtkConcreteHostError` — structured typed values.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.3 (GTK bridge and widget runtime).
