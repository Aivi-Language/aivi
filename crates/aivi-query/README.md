# aivi-query

`aivi-query` is the revision-aware analysis database shared by the CLI and LSP. It owns source
inputs, deterministic workspace/module discovery, parse and HIR caches, reverse-dependency
invalidation, entrypoint resolution, typed backend-unit queries, and stable semantic
fingerprints.

## Main surfaces

```rust
let db = aivi_query::RootDatabase::new();
let file = db.open_file("main.aivi", "value answer = 42\n");

let parsed = aivi_query::parsed_file(&db, file);
let hir = aivi_query::hir_module(&db, file);
let diagnostics = aivi_query::all_diagnostics(&db, file);
```

Other public queries include `symbol_index`, `exported_names`, `format_file`,
`reachable_workspace_hir_modules`, `whole_program_backend_unit`,
`runtime_fragment_backend_unit`, and their stable fingerprint variants.

## Invariants

- `RootDatabase` is `Send + Sync`; its state is protected by explicit read/write locks.
- Every source edit increments the file revision and the monotonic workspace revision.
- Changing or removing a file invalidates that file and all registered transitive reverse
  dependents.
- Cached values are published only if the source revision still matches after computation.
- File-to-module mapping, dependency traversal, and diagnostic ordering are deterministic.
- Parse and HIR queries return result objects containing diagnostics rather than panicking.
- Backend queries cache successful and failed lowering results against the complete semantic
  snapshot identity.
- Runtime-fragment fingerprints include captured parameter environments.
- Persistent machine-code cache namespaces remain owned by `aivi-backend`, not this crate.

`aivi-query` forwards syntax/HIR diagnostics but defines no compiler-layer diagnostic code domain
of its own. See [AIVI_RFC.md §26–27](../../AIVI_RFC.md#26-cli-reference).
