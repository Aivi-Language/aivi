# Compiling AIVI to WASM/WASI (Execution Strategy + Backend Roadmap)

This plan targets “real” deployment: run AIVI under Wasmtime/Wasmer/WasmEdge with WASI, and eventually produce WASM components for tool ecosystems (MCP) and embedding.

## Target model (recommended)

### Phase A: WASM MVP + WASI (pragmatic first)

- Emit a `wasm32-wasi` core module.
- Provide a small AIVI runtime in linear memory:
  - heap allocation
  - tagged unions (ADTs)
  - immutable records
  - strings + UTF-8
  - persistent lists
- Host syscalls via WASI and/or a thin imported ABI for effects.

This gives you “it runs” without waiting on WasmGC/component-model stability everywhere.

### Phase B: Component Model + WASI Preview 2 (tooling + embedding)

- Emit a WASM Component with WIT-defined imports/exports:
  - stable, typed interop (no manual pointer/len ABI)
  - direct embedding in Rust hosts (`wasmtime::component`)
- This becomes especially valuable for MCP tools/resources.

### Phase C: WasmGC (future performance/ergonomics)

- Once your target environments support it reliably, switch representation:
  - closures and immutable data as GC refs
  - less runtime code (or none) for allocation/GC

Keep the compiler’s typed IR and codegen interface stable so the backend can evolve.

## Compiler pipeline (concrete)

1. Parse to CST → AST.
2. Resolve modules + names → HIR.
3. Desugar HIR → Kernel IR.
4. Typecheck Kernel → Typed Kernel / THIR.
5. Closure conversion + lambda lifting (depending on backend).
6. Lower to WASM IR:
   - locals, blocks, calls, ifs, tables (for tags), memory ops.
7. Emit:
   - `*.wasm` (core module)
   - optionally `*.component.wasm` (component-wrapped module)

## Runtime representation (Phase A)

### Value layout

Pick a uniform runtime `Value` encoding that is easy to optimize later.

Option 1 (common): “boxed everything except small immediates”
- `i64` where low bits tag immediates (Int/Bool/Null-like) and pointers.
- Heap objects with a header:
  - tag / type id
  - size/len
  - payload words

Option 2 (simpler first): “box everything”
- Every value is a pointer to a heap object.
- Easier codegen; slower; acceptable for early milestones.

### GC / memory management

Stage it:
1. Bump allocator + “no collection” for tiny programs and compiler tests.
2. Add tracing GC (mark/sweep) once running real programs.
3. Optimize later (generational/copying).

### Strings and Text

For Phase A:
- Store as `{ len: u32, bytes: [u8] }` in linear memory.
- Provide intrinsics: `text.len`, `text.concat`, `text.slice`, `text.fromUtf8`, `text.toUtf8`.

When moving to the component model:
- Prefer canonical ABI strings across the boundary; keep internal representation flexible.

## Effects and WASI

### Principle

Effects should compile to imported functions/resources; typechecking enforces that they can only be used under `Effect`.

### Minimal set of effects (to unblock “real” programs)

- `Console`: `printLine : Text -> Effect Console Unit`
- `Clock`: `now : Unit -> Effect Clock Instant`
- `File`: `readText : Path -> Effect File Text`, `writeText : Path -> Text -> Effect File Unit`
- `Random`: `u64 : Unit -> Effect Random U64`

Implementation options:
- Direct WASI calls (for filesystem/clock) wrapped by AIVI runtime intrinsics.
- Or define a stable AIVI “effect import ABI” and implement it using WASI in the host.

## Code generation approaches

### Option A: Direct emission (`wasm-encoder`)

Pros:
- Minimal dependencies.
- Total control over output.
- Good for a small language.

Cons:
- You must build your own lowering and optimizations.

### Option B: Cranelift

Pros:
- Leverages a mature SSA IR and optimizations.
- Easier to get decent performance early.

Cons:
- More complex integration, and you’re still targeting WASM.

Recommendation:
- Start with `wasm-encoder` until the language stabilizes, then evaluate Cranelift if needed.

## Testing the backend

- Golden tests:
  - compile → wasm bytes → disassemble (via `wasm-tools` in dev) and snapshot
  - run in `wasmtime` and snapshot stdout/stderr
- Property tests:
  - round-trip for serialization (Text/list/records)
  - runtime invariants (tags and heap layout)

