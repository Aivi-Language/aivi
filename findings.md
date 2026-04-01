# Findings

## Matrix-derived backlog

- `compile` is still limited by the first Cranelift slice in `crates/aivi-backend/src/codegen.rs`.
- `aivi compile` stops at object code and explicitly does not produce runnable GTK binaries.
- `!|>`, `~|>`, `-|>` exist in syntax/HIR but are currently blocked from general-expression/runtime lowering.
- regex literals are validated in HIR but blocked from typed-core general expressions.
- custom `provider qualified.name` declarations are checked and lowered, but runtime provider activation currently rejects unsupported/custom providers.
- structural patch removal still reports `hir::unsupported-patch-remove`.
- runtime recurrence and accumulation already have real linked-runtime tests.
- source providers are broadly implemented, but many options remain contract-only or intentionally narrowed.
- cross-module higher-kinded execution is still narrower than same-module lowering.

## Likely dependency order

1. Inspect the blocked pipe/runtime semantics (`!|>`, `~|>`, `-|>`) because they are syntactically present but semantically incomplete.
2. Inspect custom provider runtime design because the checker/runtime contract is split.
3. Inspect patch remove/result-shape propagation because it changes typing and downstream layouts.
4. Only then widen backend/codegen coverage.

## Evidence anchors

- `manual/guide/surface-feature-matrix.md`
- `manual/guide/source-catalog.md`
- `manual/guide/typeclasses.md`
- `crates/aivi-hir/src/general_expr_elaboration.rs`
- `crates/aivi-runtime/src/providers.rs`
- `crates/aivi-runtime/src/startup.rs`
- `crates/aivi-backend/src/codegen.rs`
- `crates/aivi-cli/tests/compile.rs`
