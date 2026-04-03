# aivi-hir

## Purpose

Milestone 2 HIR boundary: name resolution, structural validation, type checking, and a suite of
elaboration passes that annotate the raw CST into a fully-resolved, typed module.
`aivi-hir` sits immediately above `aivi-syntax` in the pipeline; it owns the `Module` type that
all later compiler layers consume. It depends on `aivi-typing` for structural type analysis
(kind checking, Eq/Default derivation, decode/gate/fanout/recurrence planning).

## Entry points

```rust
// Produce a HIR Module from a parsed CST
lower_module(parsed: &ParsedModule, db: &SourceDatabase) -> LoweringResult
lower_module_with_resolver(parsed: &ParsedModule, db: &SourceDatabase, resolver: &dyn ImportResolver) -> LoweringResult
lower_structure(parsed: &ParsedModule, db: &SourceDatabase) -> LoweringResult

// Structural and type validation
validate_module(module: &Module, db: &SourceDatabase) -> ValidationReport
typecheck_module(module: &Module, db: &SourceDatabase) -> TypeCheckReport

// Elaboration passes (run in order after validation)
populate_signal_metadata(module: &mut Module, db: &SourceDatabase)
elaborate_gates(module: &Module, db: &SourceDatabase) -> GateElaborationReport
elaborate_truthy_falsy(module: &Module, db: &SourceDatabase) -> TruthyFalsyElaborationReport
elaborate_temporal_stages(module: &Module, db: &SourceDatabase) -> TemporalElaborationReport
elaborate_fanouts(module: &Module, db: &SourceDatabase) -> FanoutElaborationReport
elaborate_recurrences(module: &Module, db: &SourceDatabase) -> RecurrenceElaborationReport
elaborate_source_decodes(module: &Module, db: &SourceDatabase) -> SourceDecodeElaborationReport
generate_source_decode_programs(module: &Module, ...) -> SourceDecodeProgramReport
elaborate_source_lifecycles(module: &Module, db: &SourceDatabase) -> SourceLifecycleElaborationReport
elaborate_general_expressions(module: &Module, db: &SourceDatabase) -> GeneralExprElaborationReport

// Import resolution
resolve_imports(parsed: &ParsedModule, resolver: &dyn ImportResolver) -> Vec<ImportModuleResolution>

// Symbol extraction (for LSP)
extract_symbols(module: &Module) -> Vec<LspSymbol>
exports(module: &Module) -> ExportedNames
```

## Elaboration passes

| Pass | Purpose |
|---|---|
| `populate_signal_metadata` | Collects signal dependency edges for each item |
| `elaborate_gates` | Plans gate-stage filtering and runtime expressions |
| `elaborate_truthy_falsy` | Plans truthy/falsy branch pairs |
| `elaborate_temporal_stages` | Plans `diff` and `previous` temporal pipe stages |
| `elaborate_fanouts` | Plans list-fanout filter/join segments |
| `elaborate_recurrences` | Plans recurrence wakeup and state shapes |
| `elaborate_source_decodes` | Plans JSON/domain decode strategies for source payloads |
| `generate_source_decode_programs` | Emits concrete decode program nodes |
| `elaborate_source_lifecycles` | Plans source instance lifecycle (create/replace/teardown) |
| `elaborate_general_expressions` | Validates runtime expression sites in markup and items |

## Invariants

- `lower_module` always returns a `LoweringResult`; errors appear in its `diagnostics` field, never as panics.
- `ModuleArenas` are module-owned; IDs (`ExprId`, `BindingId`, …) are valid only within the module they came from.
- Import cycles are detected during `resolve_imports` and reported as `ImportCycle` errors.
- Every `ResolutionState` variant is explicit — unresolved names are represented as `NameError`, not as absent nodes.
- Elaboration passes return `*ElaborationReport` structs that separate `Ok` and `Blocked` outcomes; callers must not treat blocked outcomes as errors.
- `typecheck_module` only runs after structural validation has passed; running it on an invalid module is unsupported.

## Diagnostic codes

| Code | Description |
|---|---|
| `hir::applicative-cluster-mismatch` | Applicative cluster arms have mismatched shapes |
| `hir::case-branch-type-mismatch` | Case branch types are not uniform |
| `hir::circular-signal-dependency` | Signal depends on itself (direct or indirect) |
| `hir::fanout-subject-not-list` | Fanout subject expression is not a list type |
| `hir::invalid-binary-operator` | Binary operator not valid for the operand types |
| `hir::invalid-fanin-projection` | Fan-in projection path is invalid |
| `hir::invalid-pipe-stage-input` | Pipe stage receives an incompatible input type |
| `hir::invalid-projection` | Projection field does not exist on the type |
| `hir::invalid-regex-literal` | Regex literal does not compile |
| `hir::invalid-truthy-falsy-projection` | Truthy/falsy branch projection is invalid |
| `hir::invalid-type-application` | Type constructor applied to wrong number/kind of arguments |
| `hir::invalid-unary-operator` | Unary operator not valid for the operand type |
| `hir::missing-default-instance` | Required `Default` instance not found |
| `hir::missing-eq-instance` | Required `Eq` instance not found |
| `hir::missing-instance-requirement` | Class instance is missing a required member |
| `hir::non-exhaustive-case-pattern` | Case expression does not cover all variants |
| `hir::reactive-update-self-reference` | Reactive update references its own target signal |
| `hir::record-row-rename-collision` | Record row rename produces a duplicate field name |
| `hir::record-row-transform-source` | Record row transform source field does not exist |
| `hir::result-block-binding-not-result` | Result block binding is not a `Result` type |
| `hir::result-block-error-mismatch` | Result block bindings have mismatched error types |
| `hir::source-option-type-mismatch` | Source option value does not match the contract type |
| `hir::source-option-unbound-contract-parameter` | Source option references an unbound type parameter |
| `hir::truthy-falsy-branch-type-mismatch` | Truthy/falsy branches have incompatible types |
| `hir::truthy-falsy-subject-not-canonical` | Truthy/falsy subject is not a canonical boolean-like type |
| `hir::type-mismatch` | Expression type does not match the expected type |
| `hir::unknown-projection-field` | Projection field name not found on the record type |
| `hir::unknown-record-row-field` | Record row spread references a field that does not exist |
| `hir::unresolved-name` | Name could not be resolved in scope |

## RFC reference

See [`../../AIVI_RFC.md`](../../AIVI_RFC.md) §4.2 (HIR, name resolution, type checking).
