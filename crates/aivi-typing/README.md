# aivi-typing

## Purpose

Milestone 3 type-side semantics: structural kind checking, `Eq`/`Default` derivation, decode
planning, gate/fanout/recurrence planning, and source contract definitions.
This crate has **no dependencies on other AIVI crates** — it is a pure analysis library that
`aivi-hir` and later layers import to answer type-structural questions without creating a cycle.

## Entry points

```rust
// Kind checking
KindChecker::new(store: KindStore) -> KindChecker
KindChecker::check(ty: &TypeNode) -> Result<Kind, KindCheckError>

// Eq / Default derivation
EqDeriver::new(store: &TypeStore) -> EqDeriver
EqDeriver::derive(id: TypeId) -> Result<EqDerivation, EqDerivationError>

// JSON/domain decode planning
DecodePlanner::new(store: &TypeStore) -> DecodePlanner
DecodePlanner::plan(id: TypeId, mode: DecodeMode) -> Result<DecodePlanId, DecodePlanningError>

// Gate (filter) planning
GatePlanner::new(store: &TypeStore) -> GatePlanner
GatePlanner::plan(carrier: GateCarrier) -> Result<GatePlan, Vec<GateResultKind>>

// Fanout planning
FanoutPlanner::new(store: &TypeStore) -> FanoutPlanner
FanoutPlanner::plan(carrier: FanoutCarrier) -> Result<FanoutPlan, Vec<FanoutResultKind>>

// Recurrence planning
RecurrencePlanner::new(store: &TypeStore) -> RecurrencePlanner
RecurrencePlanner::plan(target: RecurrenceTarget) -> Result<RecurrencePlan, RecurrenceTargetError>
RecurrenceWakeupPlanner::plan(ctx: ...) -> Result<RecurrenceWakeupPlan, RecurrenceWakeupError>

// Source contracts
SourceContract — describes lifecycle, options, wakeup conditions for a named source provider
StructuralWalker — generic walker over TypeStore shapes
```

## Invariants

- This crate is dependency-free with respect to other AIVI crates; it must remain so.
- `TypeStore` and `KindStore` are append-only during a planning session; IDs are stable.
- `DecodePlanner` rejects types that do not have a structural decode mapping; errors are typed, not panics.
- `EqDeriver` requires all referenced types to already be in the `TypeStore`; missing types are `EqDerivationError`.
- Kind expressions are interned in `KindStore`; two structurally equal kinds share an ID.
- `StructuralWalker` prevents unbounded recursion by tracking visited type IDs.

## Diagnostic codes

This crate emits no `DiagnosticCode` values directly — errors are returned as typed `Result`
variants. `aivi-hir` translates planning errors into diagnostics using its own codes.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.1 (type system) and §4.2 (HIR type checking).
