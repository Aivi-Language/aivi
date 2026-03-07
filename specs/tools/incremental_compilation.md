# Incremental Compilation & Workspace Checking

This document defines the **incremental compiler and tooling model** for AIVI.
It applies to `aivi check`, `aivi build`, `aivi lsp`, and other tooling that
reuses frontend work across repeated analyses.

The model is deliberately **correctness-first**:

- cached artefacts are reused only when their full input fingerprint matches
- diagnostics are always attached to one coherent workspace snapshot
- cancellation may discard work, but it must not publish stale results

Reactive runtime dataflow is a separate concern. This document covers
**compile-time and editor-time dependency tracking only**; runtime signals,
subscriptions, and source-driven UI updates are specified separately.

## Goals

- avoid re-parsing and re-typechecking the whole workspace on every edit
- preserve identical answers between batch compilation and editor tooling
- make cache ownership explicit so unsaved editor state never poisons persistent
  build artefacts
- support future schema-aware sources without inventing a separate invalidation
  system for them

## Dependency graph granularity

AIVI uses a **mixed-granularity dependency graph**:

- **module granularity across files**
- **definition-group granularity within a module**

This keeps cross-module invalidation simple while still allowing same-module
reuse.

### Graph node kinds

| Node kind | Granularity | Contents | Used by |
| --- | --- | --- | --- |
| **Workspace snapshot** | whole request | workspace root, compiler options, stdlib fingerprint, open-document overlays | CLI + LSP |
| **File snapshot** | one file/URI | exact source text and file-local parse result | parser, formatter, LSP |
| **Module summary** | one module | module name, `use` declarations, exported names, decorator/source declarations, syntax-only summary needed to build the module graph | resolver, LSP workspace index |
| **Export surface** | one module | the typed public API visible to importers: exported values and schemes, types/constructors, domains, classes, instances, and exported schema summaries | typechecker, CLI, LSP |
| **Definition group** | strongly connected group of top-level definitions in one module | recursively checked binding bodies plus the local environment they require | typechecker |
| **Schema artefact** | one source/schema declaration | derived schema/config summary used by decoding, validation, source loading, or compile-time source checks | compiler + tooling |

### Granularity rules

#### Cross-module rule

Importers depend only on a module's **export surface**, not on the private bodies
inside that module.

Therefore:

- if a module body changes but its export surface fingerprint does not change,
  downstream modules do **not** become dirty merely because they import it
- if the export surface fingerprint changes, every direct importer becomes dirty,
  and the invalidation may continue transitively

#### Same-module rule

Inside one module, the recheck unit is a **definition group** rather than the
entire module body. A definition group is the strongly connected component
formed by top-level recursive references.

Therefore:

- changing one private binding dirties its own group
- other groups in the same module are rechecked only if they depend on that
  group's exported local facts
- mutually recursive bindings rise and fall together as one unit

#### Schema rule

Schema-aware source declarations participate in the same graph rather than a
parallel one.

A schema artefact depends on:

- the source declaration itself
- referenced record/type declarations
- referenced decode/validation stages
- connector configuration that changes the effective schema contract

If any of those inputs change, the schema artefact fingerprint changes and all
consumers of that artefact become dirty.

## Checkpoint and cache ownership

Every reusable artefact has one **owner**: the stage that can prove the input
fingerprint it was built from. Owners may publish immutable checkpoints for
reuse; other stages may consume them but do not mutate them in place.

| Artefact | Owner | Lifetime | Persistence |
| --- | --- | --- | --- |
| **Open-document overlay** | LSP session | until the document version changes or closes | never persisted |
| **Workspace disk index** | tooling session per workspace root | until file watching or explicit refresh invalidates it | logically disposable; may be rebuilt at any time |
| **Stdlib checkpoint** | compiler frontend version + embedded stdlib set | process/session scoped; reused across requests | may be persisted only if keyed by compiler version and stdlib fingerprint |
| **Module export checkpoint** | workspace snapshot or build session | valid only for the exact module text, options, and dependency export fingerprints used to create it | persisted only for saved on-disk inputs |
| **Definition-group/body cache** | workspace snapshot or build session | valid only inside one snapshot family | normally ephemeral |
| **Schema artefact cache** | workspace snapshot or build session | valid for the exact source/schema fingerprint | persisted only for saved inputs and stable connector metadata |

### Ownership rules

1. **Unsaved editor text is session-owned only.** It may feed LSP diagnostics and
   completions, but it must not be written into shared persistent caches.
2. **Persistent caches are derived from saved inputs only.** CLI builds and later
   editor sessions may reuse them only when the owning fingerprint matches.
3. **Checkpoints are immutable once published.** A later request creates a new
   checkpoint rather than mutating an older one in place.
4. **Cancelled or failed computations do not publish checkpoints.** Only
   successfully completed artefacts may enter a cache.

## Invalidation rules

Invalidation is driven by fingerprint changes, not by timestamps alone.

### 1. Text edits

Any text edit invalidates:

- the file snapshot for that document
- all module summaries parsed from that file
- all definition groups in those modules until the new local dependency graph is
  rebuilt

### 2. Module identity or import changes

Changes to any of the following invalidate the **module graph**:

- module name
- `use` declarations
- exported-name list
- source/decorator declarations that affect graph shape or exported artefacts

When the module graph changes:

- the touched module is dirty
- import resolution for direct dependents is dirty
- reverse-dependency traversal continues transitively if dependents' own export
  surfaces change as a consequence

### 3. Private body changes

A change confined to a private definition group invalidates:

- that group
- same-module groups that depend on it

It does **not** invalidate importers unless the module's export surface or
exported schema summary changes.

### 4. Export-surface changes

The export surface fingerprint changes when any importer-visible fact changes,
including:

- exported value names or schemes
- exported type definitions or constructors
- exported domains, classes, or instances
- exported schema summaries or compile-time source contracts

When the export surface fingerprint changes:

- all direct importers become dirty
- the dirty set propagates transitively through reverse dependencies
- each affected module is rechecked against the new upstream export fingerprints

### 5. Schema artefact changes

Schema artefact fingerprints change when any of the following change:

- schema-bearing source declarations
- referenced types or field layouts
- decode / validation pipeline stages that define the accepted shape
- connector metadata that changes static guarantees

Schema artefact invalidation follows the same rule as export surfaces:

- consumers inside the same module invalidate at definition-group granularity
- importers invalidate only when the artefact is part of the module's exported
  surface

### 6. Global environment changes

The following invalidate all typed checkpoints owned by that environment:

- compiler version changes
- stdlib fingerprint changes
- feature-flag or strictness changes that affect typing or resolution
- target configuration changes that alter compile-time source/schema behaviour

## LSP workspace recheck strategy

The LSP works over **workspace snapshots** rather than over individual files in
isolation.

Each analysis request combines:

- the current text of open documents
- the on-disk state of unopened files in the workspace
- the active compiler and LSP configuration
- reusable clean checkpoints whose fingerprints match that exact snapshot

### Recheck algorithm

On a document change, the LSP should behave as follows:

1. **Create a new snapshot token** for the workspace request.
2. **Cancel or supersede** older pending semantic work for the same workspace.
3. **Reparse the touched document** and rebuild its file and module summaries.
4. **Compute the dirty set**:
   - always include touched modules
   - include same-module dependent definition groups
   - include reverse-dependent modules only if an export surface or exported
     schema fingerprint changed
   - include any prerequisite imports needed to typecheck that dirty slice
5. **Recheck dirty modules in topological order**, reusing clean export
   checkpoints for unaffected modules.
6. **Publish diagnostics only for the current snapshot token**.

### Publish strategy

The server may publish results in stages for responsiveness, but each published
file must obey all of these rules:

- its diagnostics come from exactly one workspace snapshot
- no result from an older cancelled/superseded snapshot may be published later
- reused upstream facts must come from checkpoints whose fingerprints match that
  snapshot

This allows the LSP to be responsive without mixing old and new module facts.

### Current-file priority

For editor responsiveness, the edited file has priority:

- syntax and file-local diagnostics may appear as soon as they are known
- semantic diagnostics for the edited file should use the newest available
  coherent dirty slice
- diagnostics for untouched dependents may follow later, but when they publish
  they must replace older results for those files

## Correctness guarantees

An implementation conforms to this spec only if all of the following hold:

1. **Snapshot coherence** — every typed result is derived from one coherent
   workspace snapshot.
2. **Fingerprint reuse only** — cached artefacts are reused only when their full
   owner-defined fingerprint matches.
3. **No stale publish after cancellation** — newer edits may cancel old work, and
   cancelled work must not publish diagnostics, hovers, or semantic facts later.
4. **Batch/editor equivalence** — given the same saved workspace snapshot, batch
   compilation and LSP checking must agree on module ordering, visible exports,
   and diagnostics.

## Relevant compiler and tooling areas

The current repository areas most relevant to this architecture are:

| Area | Responsibility |
| --- | --- |
| `crates/aivi/src/surface/` | file/module parsing and syntax-level module summaries |
| `crates/aivi/src/resolver/` | module/import resolution and scope checks |
| `crates/aivi/src/typecheck/ordering.rs` | topological ordering for module rechecks |
| `crates/aivi/src/typecheck/check.rs` | export-surface checkpointing and module type setup |
| `crates/aivi/src/typecheck/elaborate.rs` | later typed elaboration checkpoints derived from module exports |
| `crates/aivi_lsp/src/state.rs` | session-owned document state, pending work, and reusable tooling state |
| `crates/aivi_lsp/src/workspace.rs` | workspace indexes, module discovery, and disk-backed view of unopened files |
| `crates/aivi_lsp/src/diagnostics.rs` | dirty-slice semantic checking and diagnostic assembly |
| `crates/aivi_lsp/src/server.rs` | request lifecycle, debounce/cancellation, and publish discipline |

These paths are informative rather than normative; alternative implementations
may organise the same responsibilities differently.

## Boundary with reactive dataflow

Incremental compilation and reactive dataflow solve different problems:

- **incremental compilation** tracks compiler and editor artefacts such as
  modules, export surfaces, definition groups, and schema summaries
- **reactive dataflow** tracks runtime values, subscriptions, derived signals,
  and UI/source update propagation

The reactive layer may reuse the words *dependency*, *memoization*, and
*invalidation*, but it must not change the ownership or correctness rules in
this document.
