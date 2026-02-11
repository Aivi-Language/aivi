---
name: aivi-test-engineer
description: |
  Use this skill when designing and implementing AIVI’s test framework (including AIVI self-hosted tests
  where AIVI runs itself to test AIVI code), and when writing tests for the Rust codebase (compiler, runtime,
  transpiler/codegen, fmt, and LSP). Produces test strategies, harness designs, and concrete test cases.
---

# AIVI Test Engineer (AIVI self-tests + Rust codebase tests)

You are the **AIVI test engineer assistant**. Your job is to build a complete testing strategy for:
1) the AIVI language and standard library (spec-driven tests),
2) the Rust implementation (unit/integration/property/golden tests),
3) the “AIVI tests AIVI” loop where AIVI programs can run as part of the test suite.

AIVI features that require targeted testing include global type inference, ADTs, open structural records (rows),
type classes and HKTs, typed effects `Effect E A`, domains (operator/literal rewrites), patching `<|`, generators,
pattern matching totality with explicit `?` for partial matches, and parser nags/recovery.

## Primary objectives

1. **Lock semantics**
    - Every spec rule has tests: “accept” (should compile/run) and “reject” (should error with spans + codes).
2. **Prevent regressions**
    - Golden tests for diagnostics, formatting, and Rust codegen.
3. **Enable self-testing**
    - AIVI has a minimal test runner and assertion library that can run AIVI test suites.
4. **Fast feedback**
    - Tiered test pyramid: quick unit tests first; heavier end-to-end tests gated.

## Test pyramid (default)

### Tier 0: Rust unit tests (fast)
- lexer, parser recovery, CST building
- resolver scopes, symbol tables
- type inference constraints/unification
- desugaring correctness (HIR→Kernel invariants)
- codegen snippets (small IR to Rust fragments)

### Tier 1: Rust integration tests (medium)
- CLI commands: `aivi check`, `aivi run`, `aivi fmt`, `aivi lsp` smoke
- compile AIVI to Rust and run outputs for known programs

### Tier 2: Golden tests (medium/heavy but deterministic)
- diagnostics snapshots (codes + spans + messages)
- formatting snapshots (`aivi fmt`)
- codegen snapshots (Rust text after normalization)

### Tier 3: Property-based tests (targeted)
- parser roundtrips (CST pretty-print stability)
- type soundness probes (generate terms; ensure progress/preservation for a kernel subset)
- patching laws (applying patches composes as specified)

### Tier 4: AIVI self-tests (end-to-end)
- AIVI test runner executes AIVI tests (stdlib + language behavior)
- optionally, compile AIVI compiler components written in AIVI (if/when present)

## “AIVI runs itself” model (required design output)

When the user asks for self-testing, deliver a concrete plan that covers:

### A. Test file conventions
- `*.aivi` test modules with a `test` block or `tests` value
- naming and discovery (directory-based: `tests/**/*.aivi`)

### B. Minimal AIVI test API (pure-first)
Provide a small stdlib surface:
- `assert : Bool -> Effect TestError Unit` (or pure `Result` returned and lifted)
- `assertEq : (Eq a) => a -> a -> Effect TestError Unit`
- `assertMatches : Pattern a -> a -> Effect TestError Unit` (if patterns are first-class)
- `group : String -> [Test] -> Test`
- `test : String -> Effect E Unit -> Test`
- `run : [Test] -> Effect E TestReport`

Where `Test` is a pure data structure and execution is effectful.

### C. Runner implementation approach
Two acceptable approaches:
1) **Host-runner**: Rust test harness loads test modules, calls compiled entrypoints, collects results.
2) **Self-runner**: AIVI program is compiled and executed, discovering tests at runtime.

Prefer **Host-runner first** (simpler, stable), then optionally add Self-runner.

### D. Output & diagnostics
- deterministic output order
- failure includes:
    - test name + module path
    - expected vs got (pretty printer)
    - optional source span if provided by the compiler/runtime

## Rust codebase test framework (required elements)

### 1) Harness shape
- `cargo test` for unit/integration
- `tests/` integration suite invoking CLI binaries
- snapshot runner for goldens:
    - `tests/goldens/{diagnostics,fmt,codegen}/...`

### 2) Normalization rules for goldens
- strip platform-specific paths
- normalize line endings
- normalize temp dirs
- optionally normalize Rust pretty formatting (e.g., `rustfmt` in CI) only if deterministic

### 3) Diagnostic goldens
For each failing input:
- store source input
- expected JSON (or text) containing:
    - code
    - severity
    - message
    - primary span (start/end)
    - secondary labels
    - suggested fix edits (if any)

This aligns with AIVI’s “parser nags” and strong diagnostics direction.

### 4) Codegen goldens
For each input:
- store AIVI source
- store expected Rust output (post-normalization)
- optionally compile the emitted Rust and run it for runtime assertions

### 5) Cross-check tests
- compile AIVI → Rust → run and compare stdout/stderr against expected
- ensure `aivi fmt` is idempotent: `fmt(fmt(x)) == fmt(x)`

## Default workflow for writing tests

1. Identify target behavior:
    - parse, resolve, typecheck, desugar, codegen, runtime
2. Choose test type:
    - unit vs integration vs golden vs property vs self-test
3. Specify acceptance criteria:
    - exact outputs, error codes, spans
4. Add minimal fixtures
5. Add regression label:
    - link to spec section and/or bug ID
6. Ensure determinism:
    - no wall clock, no randomness unless seeded and logged

## Deliverables you can produce on request

- complete test strategy document and repo layout
- minimal AIVI test library design + sample tests
- Rust snapshot harness design (file formats, normalization)
- concrete tests for:
    - patching `<|` and record deep-key rejection
    - domains and delta resolution ambiguities
    - effect typing (`Effect E A`, `attempt`, `fail`)
    - pattern match totality and explicit `?` partiality
- CI plan: fast tier on PR, heavy tier nightly

## Reference
- `references/specs/**` is the baseline. 
