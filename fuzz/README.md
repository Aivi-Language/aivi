# Fuzzing

This repo uses `cargo-fuzz` (libFuzzer) to harden the lexer, parser, formatter,
front-end pipeline, runtime, and LSP-like stack.

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
cargo install cargo-fuzz
rustup toolchain install nightly
```

## Running targets

Short smoke test (30 s per target):

```bash
cargo +nightly fuzz run parser    -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo +nightly fuzz run formatter -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo +nightly fuzz run lsp_pipeline -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo +nightly fuzz run frontend  -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
cargo +nightly fuzz run runtime   -- -max_total_time=30 -timeout=10 -rss_limit_mb=2048
```

Extended run (5 min, nightly CI):

```bash
cargo +nightly fuzz run parser -- -max_total_time=300 -timeout=30 -rss_limit_mb=4096
```

## libFuzzer flags

| Flag | Meaning |
| :--- | :--- |
| `-max_total_time=N` | Stop after N seconds |
| `-timeout=N` | Kill a single test case if it takes > N seconds (detects hangs) |
| `-rss_limit_mb=N` | Abort if RSS exceeds N MiB (detects memory explosions) |
| `-max_len=N` | Cap mutated input length (default: unbounded) |
| `-jobs=N` | Run N parallel fuzz workers |

## Corpus seeds

Seed files live in `fuzz/corpus/<target>/`. The fuzzer uses these as a starting
point and mutates from there. Good seeds = faster coverage.

## CI integration

- **PR / push** (`ci.yml`): 30 s smoke run per target with `-timeout=10 -rss_limit_mb=2048`
- **Nightly** (`nightly.yml`): 5 min extended run per target with `-timeout=30 -rss_limit_mb=4096`

Crash artifacts are saved in `fuzz/artifacts/<target>/` — use
`cargo +nightly fuzz fmt <target> <artifact>` to minimize and reproduce.
