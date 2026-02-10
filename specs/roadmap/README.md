# AIVI Roadmap

This roadmap tracks the incremental development of the AIVI language, compiler, and tooling. Each phase produces something shippable and dogfoodable.

## Guiding Principles

- **Kernel First**: Implement the Kernel first, then desugar surface language into it (`specs/03_kernel` → `specs/04_desugaring`).
- **Unified Engine**: Single parser, name resolver, and typechecker for both Compiler and LSP.
- **First-Class Domains**: Design Domains, Effects, and Resources early.

---

## Phase M0: Repo + CI Scaffolding (Complete)

- [x] Rust workspace skeleton (`crates/*`) and single `aivi` binary.
- [x] `aivi --help` with subcommands: `parse`, `check`, `fmt`, `lsp`, `build`, `run`.
- [x] "Hello world" golden test for parsing.
- [x] `cargo test` runs in CI.

## Phase M1: Parser + CST + Diagnostics (Complete)

- [x] Lexing + parsing for `specs/02_syntax` (excluding types initially).
- [x] CST preserving trivia (comments/whitespace) for IDE/Fmt.
- [x] Structured diagnostics with spans/codes.
- [x] Formatter prototype (pretty printer).
- [x] VS Code syntax highlighting.

## Phase M2: Modules + Name Resolution (Complete)

- [x] `module ... = { export ... }` and `use` imports.
- [x] Symbol tables and module graph.
- [x] `aivi check` resolving identifiers across workspace.
- [x] LSP: `textDocument/definition` (in-file).
- [x] LSP: `textDocument/definition` across modules.

## Phase M3: Kernel IR + Desugaring (Complete)

- [x] Kernel definitions matching `specs/03_kernel/01_core_terms.md`.
- [x] Surface-to-kernel desugaring pipeline.
- [x] `aivi desugar` debug output.
- [x] HIR with stable IDs for IDE.
- [x] **Acceptance**: Round-trip tests for surface features.

## Phase M4: Type System v1 (Complete)

- [x] ADTs, functions, closed records, pattern matching.
- [x] `Option`, `Result`, `List`, `Text` as library types.
- [x] Let-generalization.
- [x] Minimal traits (`Eq`, `Ord`, `Show`, `Num`).
- [x] `aivi check` with type error traces and typed holes (`_`).
- [x] Canonical type pretty-printer.
- [x] LSP: Hover types, signature help.

## Phase M5: Execution (Rust Transpilation & Native Runtime) (Complete)

- [x] `aivi build --target rustc` emits binary via Rust transpilation.
- [x] `aivi run` executes program (native/interpreter).
- [x] Basic runtime support (heap, strings, lists).
- [x] **Acceptance**: Deterministic golden tests, memory safety.

---

## Phase M6: Effects, Resources, Concurrency (In Progress)

Scope: Implement `Effect E A` semantics, structured concurrency, and resource management.

- [x] Built-in effects: `Clock`, `File`, `Random` (partial).
- [x] `specs/06_runtime/01_concurrency.md` implementation (`scope`, `par`, `race`).
- [x] Cancellation propagation rules.
- [x] `bracket`/`with` resource pattern.
- [x] Deterministic cancellation semantics.

## Phase M7: Domains + Patching (Complete)

Scope: Domain definitions, operator overloading, and patching semantics.
- [x] Domain definitions and operator interpretation (`specs/02_syntax/11_domain_definition.md`).
- [x] Patching semantics (`specs/02_syntax/05_patching.md`).
- [x] Domain-specific numeric deltas (calendar/duration/color).
- [x] Built-in sigils as domain literals (`~r`, `~u`, `~d`, `~dt`) wired through lexer/parser → HIR/Kernel and editor tooling.

## Phase M8: LSP "Daily Driver" (Complete)

Scope: Make editing AIVI comfortable for daily work.

- [x] Diagnostics (syntax/type errors).
- [x] Definition (in-file).
- [x] Formatting (via `aivi fmt`).
- [x] References (find usages).
- [x] Rename refactoring.
- [x] Hover documentation & resolved types.
- [x] Semantic tokens.
- [x] Code actions (quick fixes).

## Phase M9: MCP Integration (In Progress)

Scope: Expose AIVI modules as Model Context Protocol (MCP) tools/resources.

- [x] `aivi mcp serve` exposing `@mcp_tool` and `@mcp_resource`.
- [x] JSON Schema generation from AIVI types.
- [ ] Capability gates for unauthorized effects.

## Phase M10: Type System v2 (Long Term)

Scope: Advanced typing features.

- [ ] Row polymorphism (open records).
- [ ] Type classes (ad-hoc polymorphism).
- [ ] Higher-Kinded Types (HKTs).

---

## Detailed Plans

### Language Implementation Plan
1. **Concrete Syntax → CST**: Tokens, modules, bindings, ADTs, records, patterns. (Done)
2. **AST & Lowering**: CST→AST→HIR→Kernel pipeline. (Done)
3. **Modules**: Resolution, cycles, shadowing. (Done)
4. **Kernel IR**: The executable truth. (Done)
5. **Typechecking**: Monomorphic → Polymorphic → Traits → Effects. (Mostly Done)
6. **Diagnostics**: Error codes, labels, suggestions. (Ongoing)
7. **Patterns**: Exhaustiveness checking. (Planned)
8. **Predicates & Patching**: Central AIVI identity features. (Planned)
9. **Domains**: Custom literals and operators. (In Progress)

### Standard Library Plan
- **Phase 1**: Compiler intrinsics + thin wrappers (`aivi.std.core`).
- **Phase 2**: Implement stdlib in AIVI language where possible.
- **Phase 3**: Optimization (persistent collections, HAMT).
- **Modules**: `Prelude`, `Core` (Int/Float/Bool/Text/List), `Collections`, `Bytes`, `Json`.
- **Domains**: `Duration`, `Calendar`, `Color`, `Vector`.
- **Effects**: `Console`, `Clock`, `Random`, `File`, `Net`.

### Rust Workspace Layout
- **Binaries**: `aivi_cli`, `aivi_lsp`, `aivi_mcp`.
- **Core Libs**: `span`, `lexer`, `cst`, `parser`, `ast`, `hir`, `resolve`, `desugar`, `kernel`, `types`, `effects`.
- **Codegen**: `runtime`, `rust_codegen`.
- **Tooling**: `fmt`, `tests`, `doc`.
