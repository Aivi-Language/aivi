# Language Implementation Plan (Core Features → Order of Attack)

This file maps the spec into implementable chunks with dependencies.

## 1) Concrete syntax → CST (lossless)

Implement first (highest leverage for IDE + formatting):
- Tokens, trivia, comments, string interpolation lexing rules.
- Module headers, `use`, `export`.
- Bindings (`let` forms), lambdas, function definitions.
- ADTs and constructors.
- Records, field access, record updates.
- Pattern matching and match guards.

Notes:
- Prefer a CST that can represent “broken” syntax; the LSP should never crash on partial code.
- Produce a single `Vec<Diagnostic>` per file: errors + warnings.

## 2) AST and lowering

Create AST “views” for semantic passes:
- Expression AST with explicit nodes for operators, application, blocks.
- Type AST with arrows, type constructors, type variables, record types.
- Pattern AST for match compilation and exhaustiveness.

Lowering pipeline (recommended):
1. CST → AST (surface).
2. AST → HIR (resolved names + stable IDs).
3. HIR → Kernel (desugared, minimal constructs).

## 3) Modules and name resolution

Implement `specs/02_syntax/10_modules.md` early:
- File/module naming rules (aivi sources → module paths).
- Import resolution and export sets.
- Cycle detection; “duplicate definition” errors.
- Local and imported name resolution with shadowing rules.

IDE outputs from this stage:
- Definition locations and symbol lists for completion.

## 4) Kernel IR

Implement the Kernel IR as the compiler “truth”:
- Variables (resolved IDs, not strings).
- Literals, lambdas, applications.
- Let/rec bindings, if/then/else (or match-only).
- ADT construction and case analysis.
- Records (construction/select/update) and patch terms (if kernel supports them directly).

Treat the Kernel as “the thing you can execute/compile”.

## 5) Typechecking (staged)

### Stage 5a: monomorphic + annotations

- Parse and validate annotations.
- Typecheck without polymorphism to validate the pipeline.

### Stage 5b: rank-1 polymorphism

- Hindley–Milner style inference (Algorithm W or constraints + unify).
- Generalization at `let`.
- Instance creation at use sites.

### Stage 5c: records (closed → rows)

Start with closed records (simpler), then extend:
- Row polymorphism (open structural records).
- Field presence/absence constraints.

### Stage 5d: effects

Start with explicit annotations:
- `Effect E A` as a type constructor.
- A rule that pure code cannot call effectful primitives without being in `Effect`.
Then add inference/propagation later.

### Stage 5e: classes + HKTs (long-term)

This is substantial; gate it behind a milestone:
- Dictionary-passing translation for classes.
- Kind inference/checking for HKTs.

## 6) Patterns: exhaustiveness and decision trees

Implement:
- Pattern compilation to decision trees.
- Exhaustiveness checking (initially: best-effort with clear “unknown” cases).
- Redundancy detection (unreachable patterns).

## 7) Predicates and patching

These features are central to AIVI’s identity; implement them as early as possible *once* the kernel + typing is stable.

- Predicates (`specs/02_syntax/04_predicates.md` and kernel form).
- Patches (`specs/02_syntax/05_patching.md`) as a typed “delta” language.

## 8) Domains

Domains span parsing + typing + evaluation:
- Domain definition syntax.
- Domain-owned operator overloading / literal interpretation.
- Domain-specific “delta literals” (calendar, duration, color, style).

Pragmatic staging:
1. Hardcode a minimal `Duration` domain to validate the concept end-to-end.
2. Generalize to user-defined domains.
3. Add JSX literal desugaring via the `Html` domain.

## 9) External sources and resources

- `specs/02_syntax/12_external_sources.md` and `specs/02_syntax/15_resources.md`.
- Treat external sources as *typed inputs* that are resolved by the build system and materialize into the compiled artifact (or as WASI resources).

