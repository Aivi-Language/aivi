# Incremental Compilation & Workspace Checking

This guide explains how AIVI reuses compiler work across repeated checks without sacrificing correctness.
It applies to `aivi check`, `aivi build`, `aivi lsp`, and any other tool that needs to answer the same questions many times as files change.

Today the repository already enforces the core safety rules in this document: workspace snapshot tokens in the LSP, open-document overlays that stay session-local, module export-surface summaries for importer invalidation, and a reusable stdlib typecheck checkpoint. Some finer-grained checkpoints described below—especially definition-group and schema-artefact caches—are the target architecture and conformance model, not a claim that every optimization is already fully implemented in the current tree.

The key idea is simple: every request sees one complete **workspace snapshot**—one coherent view of files, unsaved editor text, and relevant settings at a moment in time. The compiler may reuse cached work only when the exact inputs still match that snapshot, and it may publish only results that belong to that same snapshot.

## Start here

If you want the shortest reliable mental model, keep these four rules in mind:

1. every request gets its own workspace snapshot
2. caches may be reused only when they still match that snapshot
3. private-body edits usually stay local when exported facts are explicit
4. public or conservatively inferred export changes dirty importing modules

The early sections on this page are meant to give any contributor a reliable mental model. Later sections become more implementation-oriented and spell out the cache-ownership and invalidation rules in detail.

## Mental model first

If you want the short version, think of incremental checking as taking a **fresh photo of the workspace** for each request:

- the photo includes saved files, unsaved editor buffers, and active settings
- cached work can be reused only when it still matches that exact photo
- results from an older photo must never leak into a newer one

In this document, that “photo” is called a **workspace snapshot**.

## Plain-language glossary

| Term | Plain meaning |
| --- | --- |
| **workspace snapshot** | the full input for one check or editor request: workspace root, current file contents, open unsaved overlays, and relevant settings |
| **fingerprint** | a content-based cache key for an input or checkpoint; if the fingerprint changes, cached results from the old fingerprint are stale |
| **open-document overlay** | unsaved editor text that temporarily overrides the file on disk for the current LSP session |
| **definition group** | the chunk of top-level definitions that must be checked together because they refer to one another |
| **strongly connected component (SCC)** | a group of definitions where each one can reach the others through references. For example, if function `a` calls `b` and `b` calls `a`, they form an SCC because they depend on each other. The compiler cannot safely split them apart, so they are always rechecked together |

## One concrete scenario

Suppose `Module A` exports `parseUser : Text -> Result ParseError User`, but also contains a private helper `trimText`.

- edit only `trimText` and keep `parseUser`'s explicit exported type and other importer-visible facts unchanged: the compiler can usually keep the damage inside `Module A`
- if `parseUser` is exported without an explicit type signature, that same private edit may still dirty importers conservatively because the export surface is body-sensitive
- change the exported signature or exported schema facts of `parseUser`: importers of `Module A` now need to be rechecked too
- edit two mutually recursive helpers: if they are in the same SCC, they rise and fall together as one recheck unit

## Why this exists

Incremental compilation is what makes these workflows practical:

- editing a file in an IDE without re-typechecking the whole workspace every keystroke
- running repeated checks during development without rebuilding every unchanged module
- sharing frontend results between tooling features while still keeping saved and unsaved state separate

Reactive runtime dataflow is a different system: it tracks runtime values and UI updates after a program is running. This document is only about compile-time and editor-time reuse.

## Design goals

A conforming implementation should:

- avoid re-parsing and re-typechecking the whole workspace on every edit
- give the same answers in batch compilation and editor tooling for the same saved snapshot
- keep ownership of cached artefacts explicit so unsaved editor text never contaminates persistent build caches
- let schema-aware sources participate in the same invalidation model instead of inventing a separate one

## What gets tracked

This architecture uses a mixed-granularity dependency graph:

- **module granularity across files**
- **definition-group granularity within a module**

That combination keeps cross-module invalidation understandable while still allowing useful same-module reuse.

### Graph node kinds

| Node kind | Granularity | Contents | Used by |
| --- | --- | --- | --- |
| **Workspace snapshot** | whole request | workspace root, compiler options, stdlib fingerprint, open-document overlays | CLI + LSP |
| **File snapshot** | one file or URI | exact source text and file-local parse result | parser, formatter, LSP |
| **Module summary** | one module | module name, `use` declarations, exported names, decorator and source declarations, and other syntax-level data needed to build the module graph | resolver, LSP workspace index |
| **Export surface** | one module | the typed public API visible to importers: exported values with explicit signatures or conservative inferred export fingerprints, exported types and constructors, domains, classes, instances, and exported schema summaries | typechecker, CLI, LSP |
| **Definition group** | one strongly connected group of top-level definitions in a module | recursively checked binding bodies plus the local environment they require | typechecker; target intra-module reuse boundary |
| **Schema artefact** | one source or schema declaration | derived schema or config summary used by decoding, validation, source loading, or compile-time checks | compiler + tooling |

## Granularity rules

### Cross-module rule

Importers depend on a module's **export surface**, not on its private bodies.

In practice that means:

- changing a private helper does not automatically dirty every importer
- changing a public type, value signature, constructor, domain, class, or exported schema fact does

There is one important conservative case: when an exported value or exported domain behavior has no explicit signature, the implementation may need to treat its body as part of the export fingerprint. In that case a seemingly private body edit can still dirty importers because the compiler cannot prove that the importer-visible type facts stayed the same.

### Same-module rule

Inside one module, the intended fine-grained recheck unit is a **definition group**, not the entire module body.
A definition group is the strongly connected component created by top-level recursive references. In plainer language: if top-level definitions call each other in a loop, they rise and fall together as one recheck unit.

The current repository already performs module-level incremental rechecks. Definition-group checkpointing is the finer-grained model this page defines for conforming implementations and future optimization work.

Therefore:

- changing one private binding dirties its own group
- other groups in the same module recheck only if they depend on that group's facts
- mutually recursive definitions rise and fall together as one unit

### Schema rule

Schema-aware source declarations use the same dependency graph rather than a side system. See [Schema-First Source Definitions](../syntax/external_sources/schema_first.md) for the current user-facing source forms.
A schema artefact depends on:

- the source declaration itself
- referenced record or type declarations
- referenced decode or validation stages
- connector configuration that changes the effective schema contract

If any of those change, the schema artefact fingerprint changes and its consumers become dirty.

## Checkpoints and cache ownership

Every reusable artefact has an owner: the stage that can prove which inputs produced it.
Owners may publish immutable checkpoints for reuse; consumers may read them but must not mutate them in place.

A good rule of thumb is:

- **LSP session state** owns unsaved editor buffers and may reuse them only inside that session
- **saved-on-disk builds** may populate persistent caches
- **any cache entry** is valid only for the exact snapshot family that produced it

| Artefact | Owner | Lifetime | Persistence |
| --- | --- | --- | --- |
| **Open-document overlay** | LSP session | until the document version changes or closes | never persisted |
| **Workspace disk index** | tooling session per workspace root | until file watching or explicit refresh invalidates it | disposable; may be rebuilt any time |
| **Stdlib checkpoint** | compiler frontend version + embedded stdlib set | process or session scoped; reused across requests | may be persisted only when keyed by compiler version and stdlib fingerprint |
| **Module export checkpoint** | workspace snapshot or build session | valid only for the exact module text, options, and dependency export fingerprints used to create it | persisted only for saved on-disk inputs |
| **Definition-group/body cache** | workspace snapshot or build session | valid only inside one snapshot family | normally ephemeral |
| **Schema artefact cache** | workspace snapshot or build session | valid for the exact source and schema fingerprint | persisted only for saved inputs and stable connector metadata |

### Ownership rules

1. **Unsaved editor text is session-owned only.** It may drive LSP diagnostics and completions, but it must not be written into shared persistent caches.
2. **Persistent caches come from saved inputs only.** CLI builds and later editor sessions may reuse them only when the owning fingerprint matches.
3. **Checkpoints are immutable once published.** New work creates new checkpoints instead of mutating old ones.
4. **Cancelled or failed computations publish nothing.** Only successful computations may enter a cache.

## When work becomes stale

Invalidation is based on fingerprint changes, not file timestamps alone.

### Dirty-set cheat sheet

When a file changes, ask these beginner-friendly questions first:

1. **Did the public API—or any export fingerprint the compiler must compute conservatively—change?**  
   If yes, importers become dirty.
2. **Did only a private helper change?**  
   If yes, keep the damage local to the affected definition groups.
3. **Did imports, module names, or exported names change?**  
   If yes, rebuild the module-graph slice around that module.
4. **Did schema or compiler settings change?**  
   If yes, invalidate the checkpoints that depend on those facts.

### 1. Text edits

Any text edit invalidates:

- the file snapshot for that document
- all module summaries parsed from that file
- all definition groups in those modules until the local dependency graph is rebuilt

### 2. Module identity or import changes

Changes to any of the following invalidate the module graph:

- module name
- `use` declarations
- export declarations, whether written in an export list or with inline `export`
- source or decorator declarations that affect graph shape or exported artefacts

When the module graph changes:

- the touched module becomes dirty
- import resolution for direct dependents becomes dirty
- reverse-dependency traversal continues transitively if dependents' own export surfaces change as a result

### 3. Private body changes

A change confined to a private definition group invalidates:

- that group
- same-module groups that depend on it

It does **not** invalidate importers unless the module's export surface or exported schema summary also changes.

### 4. Export-surface changes

The export surface fingerprint changes when any importer-visible fact changes, including:

- exported value names or type signatures; for inferred exports, any body detail the compiler must conservatively include
- exported type definitions or constructors
- exported domains, classes, or instances
- exported schema summaries or compile-time source contracts

When that happens:

- all direct importers become dirty
- the dirty set propagates transitively through reverse dependencies
- each affected module is rechecked against the new upstream export fingerprints

### 5. Schema artefact changes

Schema artefact fingerprints change when any of the following change:

- schema-bearing source declarations
- referenced types or field layouts
- decode or validation pipeline stages that define the accepted shape
- connector metadata that changes static guarantees

Schema artefact invalidation follows the same rule as export surfaces:

- consumers inside the same module invalidate at definition-group granularity
- importers invalidate only when the artefact is part of the module's exported surface

### 6. Global environment changes

The following invalidate all typed checkpoints owned by that environment:

- compiler version changes
- stdlib fingerprint changes
- feature flags or strictness settings that affect typing or resolution
- target configuration changes that alter compile-time source or schema behaviour

## How the LSP rechecks a workspace

The LSP works over **workspace snapshots**, not isolated single-file analyses.
Each request combines:

- current text from open documents
- on-disk state for unopened files
- active compiler and LSP configuration
- reusable clean checkpoints whose fingerprints match that exact snapshot

### Recheck algorithm

The current repository already uses snapshot tokens, export-surface summaries, a cached stdlib checkpoint, and transitive reverse-dependency rechecks for open dependents. The algorithm below describes the full model; an implementation may temporarily recheck a larger slice—such as whole modules instead of same-module definition groups—as long as it still obeys the snapshot, fingerprint, and stale-publish rules in this document.

On a document change, the server should:

1. create a new snapshot token for the workspace request
2. cancel or supersede older pending semantic work for the same workspace
3. reparse the touched document and rebuild its file and module summaries
4. compute the dirty set:
   - always include touched modules
   - include same-module dependent definition groups
   - include reverse-dependent modules only if an export surface or exported schema fingerprint changed
   - include any prerequisite imports needed to typecheck that dirty slice
5. recheck dirty modules in topological order, reusing clean export checkpoints for unaffected modules
6. publish diagnostics only for the current snapshot token

### Publish strategy

Results may arrive in stages for responsiveness, but every published file must obey these rules:

- its diagnostics come from exactly one workspace snapshot
- no result from an older cancelled or superseded snapshot may publish later
- any reused upstream facts must come from checkpoints whose fingerprints match that snapshot

### Current-file priority

For interactive editing, the changed file gets priority:

- syntax and file-local diagnostics may appear as soon as they are known
- semantic diagnostics for the edited file should use the newest available coherent dirty slice
- diagnostics for untouched dependents may follow later, but when they publish they must replace older results for those files

## Correctness guarantees

An implementation conforms to this spec only if all of the following hold:

1. **Snapshot coherence** — every typed result is derived from one coherent workspace snapshot.
2. **Fingerprint reuse only** — cached artefacts are reused only when their full owner-defined fingerprint matches.
3. **No stale publish after cancellation** — newer edits may cancel old work, and cancelled work must not publish diagnostics, hovers, or semantic facts later.
4. **Batch/editor equivalence** — given the same saved workspace snapshot, batch compilation and LSP checking agree on ordering, visible exports, and diagnostics.

## Relevant implementation areas

The current repository areas most relevant to this architecture are:

| Area | Responsibility |
| --- | --- |
| `crates/aivi/src/surface/` | file and module parsing plus syntax-level summaries |
| `crates/aivi/src/resolver/` | module and import resolution |
| `crates/aivi/src/typecheck/ordering.rs` | topological ordering for module rechecks |
| `crates/aivi/src/typecheck/check.rs` | export-surface checkpointing and module type setup |
| `crates/aivi/src/typecheck/elaborate.rs` | later typed elaboration checkpoints derived from module exports |
| `crates/aivi_lsp/src/state.rs` | session-owned document state, pending work, and reusable tooling state |
| `crates/aivi_lsp/src/workspace.rs` | workspace indexes, module discovery, and disk-backed view of unopened files |
| `crates/aivi_lsp/src/diagnostics.rs` | dirty-slice semantic checking and diagnostic assembly |
| `crates/aivi_lsp/src/server.rs` | request lifecycle, debounce, cancellation, and publish discipline |

These paths are informative rather than normative; another implementation may organize the same responsibilities differently.

## See also

- [LSP Server](lsp_server.md) for the editor-facing behavior built on these snapshot rules
- [CLI](cli.md#check) for the batch `aivi check` command and [CLI](cli.md#lsp) for `aivi lsp`
- [Schema-First Source Definitions](../syntax/external_sources/schema_first.md) for the source-declaration shapes referenced by the schema artefact rules

## Boundary with reactive dataflow

Incremental compilation and reactive dataflow solve different problems:

- **incremental compilation** tracks compiler and editor artefacts such as modules, export surfaces, definition groups, and schema summaries
- **reactive dataflow** tracks runtime values, subscriptions, derived signals, and UI or source update propagation

Both systems may talk about dependencies, memoization, and invalidation, but the runtime layer does not change the ownership or correctness rules in this document.
