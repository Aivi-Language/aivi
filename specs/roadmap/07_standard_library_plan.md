# Standard Library Plan (Modules, Bootstrapping, and WASI-facing APIs)

This plan builds a usable stdlib in parallel with the compiler, without blocking on “everything”.

References:
- `specs/05_stdlib/*`
- `specs/02_syntax/09_effects.md`
- `specs/06_runtime/01_concurrency.md`

## Design goals

- Small, orthogonal core.
- Pure by default; effectful APIs live under explicit effect modules.
- Stable “prelude” that makes common code concise.
- Decide the core type system surface area early (polymorphism + minimal traits) and reflect it in stdlib.
- Total-by-default APIs; partial operations return `Option`/`Result`, never trap.
- Portable across WASI runtimes (avoid host-specific APIs as much as possible).

## Type system surface area (decide early)

- Let-generalization policy (top-level only vs local `let` generalization).
- Minimal traits/typeclasses: `Eq`, `Ord`, `Show` (or `ToText`), plus numeric `Add`/`Sub`/`Mul` (or a small `Num`).
- Simple deriving for ADTs (no HKTs, no fancy derivation) to keep stdlib ergonomic.

## Bootstrap strategy (recommended)

### Phase 1: compiler intrinsics + thin AIVI wrappers

Implement core operations as compiler/runtime intrinsics first:
- integer/float arithmetic and comparisons
- Text primitives (length, concat, slicing, utf8 encoding)
- List primitives (construct, deconstruct, length, fold)
- Equality/ordering for primitives

Expose them as AIVI modules with small wrappers:
- `aivi.std.core` calls intrinsics but presents stable names and types.

### Phase 2: stdlib implemented in AIVI

Once codegen + runtime are stable enough:
- move implementations out of intrinsics into AIVI code
- keep intrinsics only where required for performance or platform access

### Phase 3: optimize and specialize

- add persistent maps/sets (HAMT), vectors, bytes
- specialize hot paths as intrinsics if benchmarks justify it

## Proposed stdlib module map

### Always-on (Prelude)

- `aivi.prelude` (as in `specs/05_stdlib/01_prelude.md`)

### Pure core

- `aivi.std.core`
  - `Int`, `Float`, `Bool`, `Char`, `Text`
  - `List`, `Option`, `Result`, `Tuple`
  - `Eq`, `Ord`, `Show` (or `ToText`)
  - Numeric traits: `Add`, `Sub`, `Mul` (or a small `Num`)
- `aivi.std.math` (pure numeric utilities)
- `aivi.std.collections`
  - `List` extensions
  - `Vector` (persistent)
  - `Map`, `Set` (persistent; start with ordered map or HAMT later)
- `aivi.std.bytes`
- `aivi.std.json` (encode/decode with `Result`)

### Domains (pure, but domain-owned semantics)

Based on existing sketches:
- `aivi.std.duration` (`specs/05_stdlib/03_duration.md`)
- `aivi.std.calendar` (`specs/05_stdlib/02_calendar.md`)
- `aivi.std.color` (`specs/05_stdlib/04_color.md`)
- `aivi.std.vector` (`specs/05_stdlib/05_vector.md`)
- `aivi.std.html` (`specs/05_stdlib/06_html.md`)
- `aivi.std.style` (`specs/05_stdlib/07_style.md`)

### Core data split (recommended)

Keep the always-imported surface small:
- `aivi.std.core` exposes types + minimal constructors.
- Richer APIs live in focused modules: `aivi.std.text`, `aivi.std.list`, `aivi.std.option`, `aivi.std.result`.
- `aivi.prelude` re-exports only the essentials.

### Effects model (decide early)

Pick a concrete effects story and keep the surface API small and guided:
- Algebraic effects + handlers, or an `IO`-like monad, or row-polymorphic effects.
- Capabilities stay explicit in types (e.g., `Effect Console a`).
- Provide a recommended top-level program shape even if the language is not strictly Elm.

### Effects (WASI-facing)

Keep these explicit and capability-oriented:
- `aivi.std.console` (stdout/stderr)
- `aivi.std.clock`
- `aivi.std.random`
- `aivi.std.file` (filesystem; path types; streaming later)
- `aivi.std.net.http` (optional; might be host-provided, not WASI portable yet)

### Runtime/concurrency

From `specs/06_runtime/01_concurrency.md`:
- `aivi.std.concurrent`
  - `scope`, `par`, `race`
  - `Send A`, `Recv A`, `channel.make`, `select`
- Define cancellation propagation rules (especially for `race` and `scope`).
- Provide a `bracket`/`with` pattern for resource safety.
- Be explicit about determinism guarantees (or the lack of them).

## “Primitive types” vs “stdlib types”

Make this an explicit compiler contract (and document it):
- Primitives in the compiler: `Int`, `Float`, `Bool`, `Char` (and possibly `Unit`).
- Stdlib types with compiler-known representation initially:
  - `Text`, `List`, `Option`, `Result`, records, ADTs
- Keep semantics in stdlib; intrinsics stay mechanical (codegen/pattern matching/interop only).

## Total vs partial API policy

- Any operation that can fail returns `Option` or `Result`.
- Make total/partial decisions explicit for `Text` slicing/indexing, numeric conversions, parsing, and file ops.

Over time, you can reduce the compiler’s “knowledge” if desired, but some set will remain for performance and interop.

## Packaging and build

Recommendation:
- stdlib lives in a dedicated folder (future): `stdlib/` containing `.aivi` sources.
- `aivi build` can compile stdlib first, then user modules.
- Version stdlib with the compiler initially (single repo), split later if needed.

## Testing the stdlib

- Golden tests for public APIs (types + behavior).
- Property tests (e.g., map laws, serialization round trips).
- WASI integration tests that run under Wasmtime with controlled capabilities.

## WASI strategy for stdlib APIs

Avoid leaking WASI specifics into user code:
- define AIVI-level types (`Path`, `Instant`, `Duration`, `FileHandle`)
- define effectful operations in terms of those types
- implement them using: WASI Preview 2 + component model
- keep `aivi.std.wasi.*` internal; public modules depend on narrow capability interfaces
