# aivi-gtk

## Purpose

GTK4/libadwaita widget bridge — lowers HIR markup expressions into a typed widget plan, assembles
a runtime adapter, and executes the result through an explicit GTK host boundary.
`aivi-gtk` sits above `aivi-runtime` (for reactive signal plumbing) and below the application
entry point. It does not implement a virtual DOM; widget identity and mutation are explicit.

## Entry points

```rust
// Lower a HIR markup expression into a widget plan
lower_markup_expr(expr: &hir::Expr, module: &hir::Module, db: &SourceDatabase) -> Result<WidgetPlan, LoweringError>
lower_markup_expr_with_options(expr, module, db, opts: LoweringOptions) -> Result<WidgetPlan, LoweringError>

// Lower a markup root (for top-level window construction)
lower_markup_root(root: &hir::Expr, module: &hir::Module, db: &SourceDatabase) -> Result<WidgetPlan, LoweringError>
lower_markup_root_with_options(root, module, db, opts: LoweringOptions) -> Result<WidgetPlan, LoweringError>

// Assemble a runtime adapter from a widget plan
assemble_widget_runtime(plan: &WidgetPlan, ...) -> Result<WidgetRuntimeAssembly, WidgetRuntimeAdapterErrors>
WidgetRuntimeAssemblyBuilder::new(plan: &WidgetPlan) -> WidgetRuntimeAssemblyBuilder

// GTK host: create, update, and dispose concrete GTK widgets
GtkConcreteHost::new() -> GtkConcreteHost
GtkConcreteHost::mount(assembly: &WidgetRuntimeAssembly, ...) -> Result<GtkConcreteWidget, GtkConcreteHostError>

// Widget schema lookup (static metadata)
lookup_widget_schema(kind: GtkConcreteWidgetKind) -> &'static GtkWidgetSchema
lookup_widget_schema_by_name(name: &str) -> Option<&'static GtkWidgetSchema>
supported_widget_schemas() -> &'static [GtkWidgetSchema]
```

## Invariants

- **GTK main thread invariant**: all `GtkConcreteHost` operations and any function that creates,
  mutates, or destroys a GTK widget **must** run on the GLib main thread. Calling from a worker
  thread is undefined behaviour at the GLib level.
- Widget plans are immutable after construction; runtime updates flow through `WidgetRuntimeAssembly` reactive bindings.
- `lower_markup_expr` never produces partially-lowered plans; on error the full `LoweringError` is returned.
- Property setters and event bindings are statically typed via `GtkPropertyDescriptor` / `GtkEventDescriptor`; unknown properties/events are rejected at plan time, not at mount time.
- `GtkConcreteHost` owns the lifetime of mounted `GtkConcreteWidget` values; dropping the host disposes all managed widgets.

## Diagnostic codes

This crate emits no `DiagnosticCode` values. Errors are returned as `LoweringError`,
`WidgetRuntimeAdapterErrors`, and `GtkConcreteHostError` — structured typed values.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.3 (GTK bridge and widget runtime).
