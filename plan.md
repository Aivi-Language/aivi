# AIVI roadmap sketch: becoming exceptional at typed native apps and data workflows

## Problem

AIVI already has the raw ingredients for something unusually strong: a pure functional core, explicit effects, typed external sources, a native GTK story, a formatter/LSP, and a runtime that can target real desktop apps instead of the browser. The opportunity is not to add many disconnected advanced features, but to turn those ingredients into one coherent product identity:

**AIVI should aim to become the best language for building native business apps and internal tools where UI, data, workflows, and effects are all statically modeled together.**

## Proposed approach

Sequence the work in layers that reinforce each other:

1. **Choose the wedge clearly**: typed native desktop/data apps, not “everything at once”.
2. **Strengthen the language core for app architecture**: capability-based effects and an official app model.
3. **Make data a first-class strength**: schema-first sources, transforms, validation, and typed queries.
4. **Invest in scale and ergonomics**: incremental compilation/LSP, reactive dataflow, and flagship demos.
5. **Only then add orchestration/distribution**: actors/workflows after the single-process story is excellent.

## Recommended roadmap

### Phase 0 — Foundations and focus

Goal: make later language work coherent instead of feature-driven.

- Pick **2 north-star apps** that represent the intended sweet spot:
  - a native CRUD-heavy internal tool with forms, lists, validation, and external APIs
  - an operations-style app with background sync, local cache, and long-running tasks
- Write a short **architecture/product thesis** that states what AIVI is trying to beat:
  - less boilerplate than Rust/GTK
  - stronger guarantees than TypeScript/Electron
  - lighter deployment than browser-first stacks
- Add a **performance and correctness baseline**:
  - typechecker benchmarks
  - runtime/memory stress tests
  - UI reconciliation tests
  - typed-source/schema regression tests
- Establish an **experimental feature process**:
  - spec first
  - feature gate / experimental namespace
  - integration test requirement
  - LSP/docs propagation checklist

Why this comes first:

- It prevents the language from growing in several unrelated directions.
- It gives future design work a concrete bar: “does this help the north-star apps?”

### Phase 1 — Capability-based effects

Goal: make side effects precise, testable, and composable.

Language direction:

- Introduce a **capability story** on top of `Effect E A`.
- Model file, network, db, clock, randomness, process, UI, and cancellation access as explicit capabilities instead of ambient authority.
- Add a design for **effect handlers / interpreters** so tests and local simulations can replace real IO cleanly.
- Make capability errors and missing-permission diagnostics first-class in the compiler and LSP.

Why this matters:

- This becomes the foundation for UI commands, typed connectors, workflows, and safer embedding.
- It gives AIVI a stronger “correct-by-construction application code” identity than most desktop languages.

Success criteria:

- The same app logic can run against real connectors or test interpreters without manual dependency plumbing.
- Unsafe/global-style runtime access becomes the exception, not the norm.

### Phase 2 — Official native app architecture

Goal: turn GTK support into a flagship workflow, not just a set of primitives.

Language/runtime/tooling direction:

- Stabilize a single blessed **model / update / view / command / subscription** pattern.
- Make `gtkApp` and `signalStream` converge on one official architecture instead of parallel idioms.
- Add typed support for:
  - commands that produce effects
  - subscriptions/event sources
  - form state and validation
  - widget identity and reconciliation
  - long-running tasks with cancellation and progress reporting
- Improve LSP support around GTK/UI constructs:
  - better diagnostics on signal wiring
  - hover docs for widget attributes/signals
  - navigation between `Msg`, `toMsg`, `update`, and view code

Why this matters:

- This is where AIVI can be genuinely differentiated: Elm-like ergonomics with native GTK binaries.
- A polished app architecture makes the language feel opinionated and complete.

Success criteria:

- A real multi-screen GTK app can be built without callback spaghetti or runtimely-wired message names.
- The official app pattern is simple enough to teach and strong enough for larger apps.

### Phase 3 — Schema-first sources and typed data pipelines

Goal: make external data one of AIVI’s signature strengths.

Language/runtime direction:

- Evolve `Source` / `load` into a fuller **schema-first data system**.
- Allow source definitions to carry:
  - schema information
  - validation rules
  - decoding/transform hints
  - caching/sync policy
  - provenance/observability metadata
- Add language-level composition for:
  - transforms
  - validation accumulation
  - retries/timeouts/backoff
  - data normalization
  - source mocking in tests
- Keep the “return type drives decoding” story, but expand it into a more complete pipeline model.

Why this matters:

- Native internal tools live or die by data integration.
- AIVI can stand out if API/db/file/env ingestion feels like using typed functions, not a tangle of ad hoc clients and decoders.

Success criteria:

- Building a REST/file/database-driven app requires minimal manual decoding glue.
- Schema changes produce precise compile-time or test-time feedback.

### Phase 4 — Incremental and reactive infrastructure

Goal: make the language scale to larger apps and feel live during development.

Compiler/tooling direction:

- Add **incremental typechecking and compilation caching** where practical.
- Track dependency graphs between modules, defs, and perhaps schema-driven artifacts.
- Improve LSP responsiveness by rechecking only affected regions/modules.

Runtime/language direction:

- Introduce a principled **reactive dataflow layer**:
  - derived values/signals
  - memoized computations
  - invalidation propagation
  - source-driven updates feeding the UI model safely
- Keep this integrated with the effect/capability model rather than bolting on a separate FRP language.

Why this matters:

- It addresses the current scale risks directly.
- It also unlocks “data updates UI naturally” without abandoning explicitness.

Success criteria:

- Editing larger projects remains responsive.
- UI state and external data can be wired reactively without hidden mutation or tangled callback graphs.

### Phase 5 — Typed query/database DSL

Goal: make AIVI especially compelling for business software.

Language/runtime direction:

- Add a statically checked query DSL for relational-ish data access.
- Support compile-time validation of selected fields, filters, joins, grouping, and result shapes.
- Integrate queries with:
  - AIVI records/types
  - source schemas
  - effects/capabilities
  - migrations or schema snapshots
- Decide early whether the initial version is:
  - embedded SQL with checking
  - a native AIVI query DSL
  - or a hybrid

Why this matters:

- Business apps need trusted data access more than exotic language features.
- Done well, this combines perfectly with typed sources, validation, and native UI.

Success criteria:

- A developer can build a serious CRUD/reporting app with far fewer runtime shape mistakes.
- Queries compose naturally with records, forms, and source-derived types.

### Phase 6 — Typed workflows and actors

Goal: extend AIVI from desktop/data apps into orchestration-heavy systems.

Language/runtime direction:

- Add typed actors/processes with explicit channels and protocol-safe messaging.
- Design workflow primitives for:
  - retries
  - deadlines/timeouts
  - cancellation propagation
  - compensation/sagas
  - persistence checkpoints
- Keep the initial scope narrow: background jobs and long-running app workflows before distributed systems.

Why this comes later:

- This is high leverage only after effects, app architecture, and data modeling are solid.
- Otherwise it risks pulling the language away from its strongest near-term identity.

Success criteria:

- AIVI apps can model background sync, scheduled jobs, and business workflows without ad hoc thread/process code.
- Messaging stays typed and protocol mistakes are caught early.

## Recommended first execution slice

If we want the highest payoff with the least fragmentation, the first real push should be:

1. **Phase 0 foundations**
2. **Phase 1 capability-based effects**
3. **Phase 2 official GTK app architecture**
4. **Phase 3 schema-first data pipelines**

That bundle creates a very coherent message:

> “AIVI is the language for building native apps where permissions, UI state, and external data are all typed end-to-end.”

The incremental engine, query DSL, and workflows should follow after that core story is already convincing.

## Key design constraints

- **Spec-first always**: every feature must start in `specs/` and propagate through compiler/runtime/LSP/tests/docs.
- **Prefer one blessed path** over several overlapping ways to solve the same problem.
- **Keep effects explicit** even when adding reactive or workflow features.
- **Avoid hidden mutation** and preserve the language’s functional identity.
- **Make tooling part of the feature**: diagnostics, hover docs, formatting, and examples should ship with the language change.
- **Use flagship demos as forcing functions** so features are judged against real app ergonomics, not only theoretical elegance.

## Notes

- The main risk is trying to add capabilities, FRP/reactivity, query DSLs, actors, and workflow persistence all at once.
- AIVI will likely win more by being **excellent at one coherent category** than by being broadly “advanced”.
- The strongest coherent category appears to be:
  - **typed native desktop applications**
  - **data-heavy internal/business software**
  - **safe long-running workflows**

## Todo mapping

- `roadmap-foundations`
- `effect-capabilities`
- `gtk-app-architecture`
- `typed-sources-pipeline`
- `incremental-reactive-engine`
- `typed-query-dsl`
- `workflow-actors`