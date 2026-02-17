<p align="center">
  <img width="128" height="128" alt="image" src="https://github.com/user-attachments/assets/f2815310-861c-454d-a37c-9dd4f4bd0aed" />
</p>

# AIVI Language Specification & Implementation

> [!WARNING]
> The whole project has been vibe-coded and is not fit for production as of today.
> I just want to see if I can get my own language up and running by applying my experience
> and expertise to guide LLM models and to learn which ways are the best.

AIVI is a type-safe functional language targeting WebAssembly, featuring **global type inference**, **open structural records** (row polymorphism), **type classes with higher-kinded types (HKTs)**, **typed effects** (`Effect E A`), and **algebraic data types (ADTs)**.

**AIVI v0.1** implements a CST→AST→HIR→Kernel pipeline with native Rust runtime execution.

This repository contains:
- **Language specification** (normative semantics, type system, desugaring rules)
- **Rust implementation** (compiler pipeline + runtime)
- **Rust codegen** (typed Kernel → Rust AST emission)
- **VSCode extension** (syntax highlighting, type checking, quick docs etc)
- **Documentation** (specs, online docs, local build)
- **CLI** (project scaffolding, building, introspection)

## Documentation

- **Specification entry**: [`specs/README.md`](specs/README.md)
- **Online documentation**: [mendrik.github.io/aivi](https://mendrik.github.io/aivi/)
- **Local docs build**: `cd specs && pnpm docs:dev` (or `pnpm docs:build`)

## Type System & Language Features

AIVI provides:

1. **Global type inference** with let-generalization
2. **Algebraic data types (ADTs)** with pattern matching via `?` operator
3. **Open structural records** with row polymorphism (extend/shrink via patching `<|`)
4. **Type classes and higher-kinded types** (Fantasy Land algebraic hierarchy)
5. **Typed effects** (`Effect E A`) with compositional error handling (`bind`, `pure`, `fail`)
6. **Domains as static rewrites** — operator/literal resolution chains (e.g., calendar deltas, color ops)
7. **Patching operator `<|`** — desugars to nested updates/removals, supporting deep-key literals with `{ a.b.c: value }`

## Syntax Examples

> The snippets below use AIVI syntax; GitHub highlighting is approximate.

### Counter model with ADTs and patching

```aivi
module demo.counter
export Model, Msg, init, update, view

Model = { count: Int, step: Int }
Msg = Inc | Dec | SetStep Int

init : Model
init = { count: 0, step: 1 }

update : Msg -> Model -> Model
update msg model =
  msg ?
  | Inc       => model <| { count: _ + model.step }
  | Dec       => model <| { count: _ - model.step }
  | SetStep s => model <| { step: s }

renderCount : Model -> Text
renderCount model = "Count: {model.count}"
```

### Domain-directed deltas (static operator rewrites)

```aivi
deadline = now + 2w + 3d             // Calendar domain
shade    = { r: 255, g: 85, b: 0 } + 10l - 30s  // Color domain
width    = 100%                      // Style percentage
height   = 100svh                    // Style viewport
```

### Typed effects with fallback

```aivi
main = effect {
  txt <- load (file.read "missing.txt") or "(missing)"
  print txt
}
```

### I18n sigils (type-checked placeholders)

```aivi
welcomeKey = ~k"app.welcome"
welcomeMsg = ~m"Hello, {name:Text}!"
```

## Compiler Pipeline

The AIVI compiler implements a multi-stage pipeline:

1. **Lexer** → Token stream
2. **Parser** → Concrete Syntax Tree (CST)
3. **AST lowering** → Abstract Syntax Tree (AST)
4. **Resolution** → High-level Intermediate Representation (HIR) with symbol IDs
5. **Desugaring** → Kernel IR (minimal core calculus: fold + generators model)
6. **Type inference** → Constraint generation and unification (supports row polymorphism, classes, effects)
7. **Runtime execution** (v0.1 native) or **Rust codegen** (experimental)

Additional tooling:
- **Formatter** (`aivi fmt`)
- **Test runner** (`aivi test`) for integration tests (`@test` definitions)
- **LSP server** (`aivi_lsp`) for editor integration

## Development

- **Run workspace tests**: `cargo test --workspace`
- **Check GitHub Actions health (via gh)**: `scripts/gh-actions-check.sh`

## Feedback & Contributions

If you identify:
- **Type-soundness issues** (principal types, constraint generation bugs)
- **Spec-code divergence** (documented but unimplemented, or vice versa)
- **Unclear semantics** (ambiguous desugaring, missing inference rules)
- **Ergonomic problems** (confusing error messages, parser nags, type errors)

Please:
- Open an issue/PR with a **minimal counterexample** (runnable `.aivi` snippet)
- Reference specific spec sections in [`specs/`](specs/)
- For type system issues, include expected vs. actual inferred types

## CLI Usage (Experimental)

The `aivi` CLI supports project scaffolding, building, and introspection:

### Project management
```sh
cargo install aivi
aivi init my-app --bin          # or --lib
cd my-app && aivi build
cd my-app && aivi run
aivi install aivi-foo@^0.1      # dependency management
aivi package && aivi publish --dry-run
```

### Compiler introspection
```sh
aivi kernel integration-tests/syntax/ir_dump_minimal.aivi    # Dump Kernel IR (JSON)
aivi rust-ir integration-tests/syntax/ir_dump_minimal.aivi   # Dump Rust IR (JSON)

# Direct rustc invocation with custom flags
aivi build integration-tests/syntax/ir_dump_minimal.aivi --target rustc \
  --out target/aivi-rustc/hello_bin -- -C opt-level=3
```

**Implementation Note**: Generated Rust code is emitted to `target/aivi-gen/` (managed builds) or `target/aivi-rustc/` (direct `rustc` target).

## Todo

- mcp server doesn't start
- some imports report problems List.partition
- LSP server should suggest auto import

## License

[License information to be determined]
