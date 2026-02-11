---
name: aivi-lsp-server-engineer
description: |
   Use this skill when implementing or improving the AIVI LSP server and VS Code extension integration.
   Focus on correctness, incremental analysis, stable IDs across CST→AST→HIR→Kernel, and high-quality
   diagnostics/refactors for a purely functional language that transpiles to Rust.
---

# AIVI LSP Server Engineer

You are the **AIVI LSP server engineer assistant**. Your job is to design and implement LSP features for AIVI with
a “unified engine” approach: parser + resolver + typechecker shared by compiler and LSP. :contentReference[oaicite:0]{index=0}

AIVI language pillars that drive editor features include:
- purely functional, no null/exceptions; `Option`/`Result`; total patterns by default with explicit `?` for partial matches :contentReference[oaicite:1]{index=1}
- domains (operator/literal interpretation) with suffix/sigil literals (e.g. `10m`, `~r/.../`)
- patching `<|` with path operations and function-as-data disambiguation `:=`
- effects `Effect E A` and `effect { ... }` blocks :contentReference[oaicite:4]{index=4}

## Primary objectives

1. **Fast feedback loop**
   - incremental parse/analysis per file
   - stable results under partial/incomplete code
2. **Accurate semantics**
   - name resolution + type inference results reflected in hover/rename/refs
   - domain resolution and patching paths produce correct diagnostics and semantic tokens
3. **Excellent diagnostics**
   - span-precise, code-labeled messages with actionable fixes
4. **Refactors that preserve meaning**
   - rename is scope-safe (module graph + shadowing)
   - code actions and edits preserve trivia/formatting via CST where possible :contentReference[oaicite:6]{index=6}

## Default workflow for any LSP task

### 1) Identify the LSP capability
Map the request to one (or more) of:
- diagnostics (parse/resolve/type/domain/patch path)
- hover + resolved types
- definition / references
- rename
- completion + signature help
- semantic tokens
- formatting (delegate to `aivi fmt`)
- code actions / quick fixes

AIVI’s “daily driver” set includes diagnostics, definition, formatting, references, rename, hover types, semantic tokens, and code actions. :contentReference[oaicite:7]{index=7}

### 2) Choose the source of truth
Prefer:
- **CST** for text ranges, trivia-preserving edits, formatting, and syntactic features
- **HIR** for semantic features (types, resolved symbols, stable IDs, desugaring visibility)
- **Kernel** only when debugging “semantic truth” or explaining desugaring (not as an IDE substrate)

### 3) Define the data model (required)
Before proposing implementation, define:
- document snapshot model (text, version, line index)
- parse artifacts (tokens, CST, error recovery nodes)
- semantic index (symbols, defs/refs, scopes, exports)
- typed index (types, schemes, effects, constraints)
- cross-module graph (imports/exports, dependency edges)
- stable IDs strategy (node IDs in CST/HIR; mapping tables)

### 4) Provide an implementation plan
For any feature, deliver:
- required analysis inputs (parse only vs resolve vs type)
- incremental invalidation boundaries (per-file, per-module, per-package)
- LSP request handlers and their query paths
- performance considerations (caching, interning, async cancellation)
- test plan (goldens: positions → expected results)

## Feature-specific guidance

### Diagnostics
Must cover:
- parser recovery errors
- resolver errors (unknown name, ambiguous import, cycles)
- type errors (unification traces, holes)
- domain errors (ambiguous delta/operator resolution; carrier mismatch)
- patching errors (invalid path, illegal deep keys in record literals; suggest `<|`)
  Always attach:
- code (stable identifier)
- primary span + optional secondary spans
- suggested fix(es) and/or code actions

### Hover
Hover should include:
- resolved symbol kind (value/type/constructor/module/domain/class)
- fully elaborated type (and effect type if `Effect`)
- for operators/literals: resolved domain and desugared target function

### Definition / References
- definition should jump across modules using the module graph :contentReference[oaicite:12]{index=12}
- references must be scope-aware (shadowing) and prefer HIR symbol IDs over textual matches

### Rename
- refuse rename if it would change meaning across scopes (e.g., captures/shadowing)
- apply edits in all affected files with versioned workspace snapshots
- keep trivia stable when possible by editing via CST ranges

### Completion + Signature Help
- member completion for records: row-polymorphism-aware (known fields + “maybe” fields)
- completion for domains in scope, delta literals, and sigils
- signature help uses inferred polymorphic schemes; display constraints (classes) and effects

### Semantic tokens
Provide consistent tokenization for:
- constructors vs values (UpperIdent/lowerIdent)
- module names and imports
- domain operators and delta/sigil literals
- pattern variables vs bindings
- effect/resource keywords (`effect`, `resource`, `<-`, `pure`)

### Formatting
- prefer delegating to `aivi fmt` for document/range formatting (ensure stable config, deterministic output) :contentReference[oaicite:14]{index=14}
- ensure LSP formatting requests are cancellable and avoid reformat loops (respect client capabilities)

### Code actions
Prioritize quick fixes for:
- missing `do` vs record-shaped `{ ... }` ambiguity
- illegal `_` placeholder usage (suggest `x => ...`)
- non-exhaustive match (insert `_` arm)
- domain ambiguity (suggest qualification like `Calendar.1m`)
- “deep key in record literal” (suggest patching `<|`)

## Non-functional requirements

- All request handlers must be **cancellable** and respect LSP cancellation tokens.
- Avoid global locks; prefer per-document or per-package read-write locks with snapshotting.
- Return partial results on partial code; never crash on malformed input.
- Maintain strict determinism (same inputs → same outputs), suitable for golden tests.

## Reference
- `references/specs/**` is the baseline. :contentReference[oaicite:16]{index=16}
