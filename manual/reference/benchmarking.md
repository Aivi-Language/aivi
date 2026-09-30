# Benchmarking

AIVI performance work is measurement-driven. The repository keeps Criterion suites in the
`aivi-benches` package and exposes lightweight phase timings through the CLI.

## Benchmark suites

| Suite | Measures |
| --- | --- |
| `parser` | Snake/Reversi parsing and a larger lexer workload |
| `typecheck` | Frontend type checking, including a repeated large input |
| `pipeline` | Query reuse, incremental invalidation, workspace reachability, lowering, fingerprints, AOT, cold/warm JIT, and native list construction |
| `runtime` | Signal graph construction, sparse/idle ticks, propagation, committed-value collection, derivation, linking, map hashing/lookups, and snapshot copying/release |
| `lsp` | Diagnostics, formatting, semantic-token full/range/delta, references, and UTF-16 edits |

Run one suite from the repository root:

```sh
cargo bench -p aivi-benches --bench pipeline
```

Filter to one benchmark when iterating:

```sh
cargo bench -p aivi-benches --bench pipeline -- cached_parse_and_hir
```

Run all suites only when you need a broad comparison:

```sh
cargo bench -p aivi-benches
```

Criterion writes reports below `target/criterion`. Do not commit generated reports.

The `runtime_maps` group measures hashing whole maps, scalar-key lookups, and lookups using
a map as a key, with 4, 64, and 1,024 entries. `hash` measures repeated hashing of the same
immutable map; `cold_hash` builds a fresh map before each timed hash. Construction happens
outside the timed region for both. `construct`, `construct_and_hash`, and `clone` measure
allocation and copying separately, including dropping each completed map.
The lookup benchmarks perform 32 lookups in independently constructed maps per iteration
and report throughput per lookup, reducing sensitivity to one randomized table layout.
Nested-key cases keep two distinct keys in the outer map: `IndexMap` bypasses hashing for
singleton lookups, which would otherwise conceal the cost this benchmark needs to measure.
Map hashing must ignore insertion order because equal maps can have different insertion orders;
the scalar-key workload guards the common lookup path independently of whole-map hashing.

The `runtime_snapshots` group measures copying and releasing boundary snapshots: one scalar,
a 64-element list, a 64-entry map, and 32 lawful `join (pure task)` layers. Each source value
is built outside the timed region. These cases guard shallow-copy costs alongside task-plan
ownership costs. Separate small-stack regression tests cover 10,000 task layers and 20,000
mixed value layers; timing a shallow plan alone cannot establish stack safety.

The `lsp_reference_index_rebuild` group measures reference requests after an unrelated document
edit invalidates the workspace index. It keeps the large document's HIR cached and uses 256,
1,024, and 4,096 references to one symbol. Each timed request gets its own preceding edit;
batching several edits before several requests would mostly measure cache hits instead of rebuilding.

The `native_flat_map_input` group executes native kernels with alternating, prebuilt input
lists to bypass last-call result caching. It measures marshalling, list construction, and result
unmarshalling at 256, 1,024, and 4,096 elements. `native_flat_map` additionally constructs its
input with the ambient range helper; that helper has its own repeated-append cost. The `sparse_ticks` group holds the changed subgraph at one edge
while increasing unrelated inputs from 256 to 16,384; it measures both publication and idle
ticks after initialization. These expose scaling costs that a small dense graph can hide.
The `committed_values` group forces collection of retained text lists; it measures collector
work separately from evaluation and publication. `cold_full_lowering` exercises HIR preparation
and every lowering layer through backend IR. `hir_elaboration` compares separate preparation
for each pass with shared preparation in the same executable.

When changing scheduler traversal, also run `propagate_chain_256` and `propagate_fanout_256`
so sparse improvements do not conceal a dense-propagation regression.


## Compare a change

Use the same machine, power profile, toolchain, feature set, and background workload for both
measurements. Warm the machine first, then save a baseline:

```sh
cargo bench -p aivi-benches --bench runtime -- --save-baseline before
# apply the implementation change
cargo bench -p aivi-benches --bench runtime -- --baseline before
```

Record the exact benchmark name, command, compiler revision, median estimate, confidence interval,
and any outliers. For latency-sensitive work, also record p95 from a representative repeated
workload rather than inferring it from a single Criterion estimate.

## Memory, startup, and size

Use the release profile for user-visible measurements:

```sh
/usr/bin/time -v target/release/aivi check demos/snake.aivi
/usr/bin/time -v target/release/aivi execute path/to/program.aivi
stat --printf='%s\n' target/release/aivi
```

`Maximum resident set size` from `/usr/bin/time -v` is the peak RSS for that process. Repeat the
command enough times to separate cold filesystem effects from steady-state behavior. Keep cold and
warm results separate.

For compiler phase orientation:

```sh
target/release/aivi check --timings demos/snake.aivi
```

The timing report identifies where to investigate; the benchmark suite is the regression test.

## Performance change policy

An optimization should preserve semantic output and include:

- a reproducible before/after command
- median and p95 latency where the workload is latency-sensitive
- allocation or peak-RSS evidence for memory claims
- output size for code-generation or packaging changes
- correctness tests for any algorithm or representation that changed

Noise-sized changes are not evidence. Keep an optimization only when the measured improvement is
repeatable or the change removes a proven scaling hazard without weakening an invariant.
