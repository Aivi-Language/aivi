# AIVI (draft language spec)

> [!NOTE]
> **AIVI v0.1** is primarily an interpreted language embedding a CST-to-Kernel pipeline.
> Experimental native Rust codegen exists, but coverage is smaller than the interpreter today.
> See [Missing Features](specs/missing_features_v0.1.md) for current implementation status.

AIVI is a high-integrity functional language aimed at WebAssembly. This repo contains the **v0.1 Rust implementation** (interpreter) and the language specification.

- Read the spec entrypoint: `specs/README.md`
- Browse the docs index: [Full specification](https://mendrik.github.io/aivi/)
- Build the docs site: `cd specs && pnpm docs:dev` / `pnpm docs:build`

## Syntax sketch (very early)

The snippets below are written in AIVI syntax; GitHub highlighting is approximate.

```haskell
module demo.counter
export Model, Msg, init, update, view

Model = { count: Int, step: Int }
Msg = Inc | Dec | SetStep Int

init : Model
init = { count: 0, step: 1 }

update : Msg -> Model -> Model
update msg model =
  msg ?
  | Inc        => model <| { count: _ + model.step }
  | Dec        => model <| { count: _ - model.step }
  | SetStep s  => model <| { step: _ <- s }

// Pipes and a few “ligature-friendly” operators: -> => |> <| ?? <= >= != && ||
renderCount = model =>
  model.count
    |> toText
    |> "Count: _"

// Domain-directed deltas (examples from the spec’s stdlib ideas)
deadline = now + 2w + 3d
shade    = { r: 255, g: 85, b: 0 } + 10l - 30s
width    = 100%   // typed Style percentage delta
height   = 100svh // typed Style viewport delta
```

Effect fallback with `or` (fallback-only sugar):

```aivi
main = effect {
  txt <- load (file.read "missing.txt") or "(missing)"
  print txt
}
```

I18n key + message sigils (placeholder types are checked):

```aivi
welcomeKey = ~k"app.welcome"
welcomeMsg = ~m"Hello, {name:Text}!"
```

## Feedback

If you see type-soundness issues, unclear semantics, bad ergonomics, or “this desugaring can’t work” problems:

- Open an issue / PR with a minimal counterexample.
- Point to a specific page in `specs/` (or propose a rewrite).

## CLI (experimental)

The `aivi` CLI can scaffold and build Cargo-backed AIVI projects (generated Rust goes in `target/aivi-gen/`):

- `cargo install aivi`
- `aivi init my-app --bin` (or `--lib`)
- `cd my-app && aivi build`
- `cd my-app && aivi run`
- `cd my-app && aivi install aivi-foo@^0.1`
- `cd my-app && aivi package`
- `cd my-app && aivi publish --dry-run`

It also has compiler-introspection and a direct `rustc` path:

- `aivi kernel examples/10_wasm.aivi` (dump Kernel IR as JSON)
- `aivi rust-ir examples/10_wasm.aivi` (dump Rust IR as JSON)
- `aivi build examples/10_wasm.aivi --target rustc --out target/aivi-rustc/hello_bin -- -C opt-level=3`
