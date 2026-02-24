# Fuzzing

This repo uses [`bolero`](https://github.com/camshaft/bolero) to harden the lexer, parser,
formatter, front-end pipeline, runtime, and LSP-like stack.

Bolero is a unified fuzzing and property-testing front-end that supports
multiple engines (libFuzzer, AFL, honggfuzz) and works on stable Rust for
regular `cargo test` runs.

## Targets

| Target | What it exercises | Key invariants |
| :--- | :--- | :--- |
| `parser` | Lexer + CST parser (+ parse-after-format on valid inputs) | Never panic/UB; always returns tokens + AST or diagnostics |
| `formatter` | `format_text` / `format_text_with_options` | Never panic; output ≤ 8× input + 4 KiB; idempotent; preserves validity |
| `frontend` | Parse → arena lowering → resolve → typecheck (+ stdlib check) → desugar → kernel | Never panic on well-typed input |
| `runtime` | Full pipeline + native execution with fuel budget | Never panic/hang (fuel-capped at 25 000 steps) |
| `lsp_pipeline` | Parse → resolve → typecheck → format → render diagnostics | Never panic; simulates LSP didOpen/didChange → formatting flow |

## Local setup

```bash
cargo install -f cargo-bolero
```

## Running targets

### As property tests (stable Rust, no special tooling)

```bash
cargo test -p aivi-fuzz
```

### Fuzz with libFuzzer (requires nightly + cargo-bolero)

Short smoke test (30 s per target):

```bash
cargo bolero test -p aivi-fuzz parser::parser       --engine libfuzzer -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo bolero test -p aivi-fuzz formatter::formatter  --engine libfuzzer -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo bolero test -p aivi-fuzz lsp_pipeline::lsp_pipeline --engine libfuzzer -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo bolero test -p aivi-fuzz frontend::frontend    --engine libfuzzer -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo bolero test -p aivi-fuzz runtime::runtime      --engine libfuzzer -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
```

Extended run (5 min):

```bash
cargo bolero test -p aivi-fuzz parser::parser --engine libfuzzer -- -max_total_time=300 -timeout=30 -rss_limit_mb=4096
```

### Fuzz with AFL

```bash
cargo bolero test -p aivi-fuzz parser::parser --engine afl --runs 100000
```

## Corpus seeds

Seed files live in `fuzz/corpus/<target>/`. Bolero automatically discovers
and uses these as starting points.

## CI integration

- **PR / push** (`ci.yml`): `cargo test -p aivi-fuzz` (property-test mode)
- **Nightly** (`nightly.yml`): Extended libFuzzer runs per target
