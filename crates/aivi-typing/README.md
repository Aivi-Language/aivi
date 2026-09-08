# aivi-typing

## Purpose

Milestone 3 type-side semantics: structural kind checking, `Eq`/`Default` derivation, decode
planning, gate/fanout/recurrence planning, and source contract definitions.
This crate has **no dependencies on other AIVI crates** — it is a pure analysis library that
`aivi-hir` and later layers import to answer type-structural questions without creating a cycle.

## Entry points

- [`KindChecker`](src/kind.rs) is a stateless checker with `infer`, `expect_kind`, and solution-producing variants over a `KindStore`.
- [`EqDeriver`](src/eq.rs) analyzes structural equality/default evidence.
- [`DecodePlanner::plan`](src/decode.rs) takes a `TypeStore`, `TypeId`, and `DecodeMode`, returning a `DecodeSchema`.
- [`GatePlanner`](src/gate.rs), [`FanoutPlanner`](src/fanout.rs), and [`RecurrencePlanner`](src/recurrence.rs) plan the corresponding carrier operations.
- `RecurrenceWakeupPlanner` checks source wakeup evidence.
- [Source contracts](src/source_contracts.rs) define provider arguments, options, and wakeups.
- [`StructuralWalker`](src/walker.rs) supplies an explicit worklist and assembly stack.

The source modules define the exact signatures; these planners do not share a universal constructor or result type.

## Invariants

- This crate is dependency-free with respect to other AIVI crates; it must remain so.
- `TypeStore` and `KindStore` are append-only during a planning session; IDs are stable.
- `DecodePlanner` rejects types that do not have a structural decode mapping; errors are typed, not panics.
- Referenced type IDs must belong to the provided `TypeStore`; structural failures are reported through typed derivation errors.
- Kind-expression IDs belong to their `KindStore`; do not transfer them between stores.
- `StructuralWalker` uses explicit stacks instead of Rust recursion. Individual planners own cycle detection and must preserve the walker's assembly-stack preconditions.

## Diagnostic codes

This crate emits no `DiagnosticCode` values directly — errors are returned as typed `Result`
variants. `aivi-hir` translates planning errors into diagnostics using its own codes.

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §3.1 (type system) and §4.2 (HIR type checking).
