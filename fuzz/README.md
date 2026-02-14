# Fuzzing

This repo uses `cargo-fuzz` (libFuzzer) to harden the lexer/parser and front-end pipeline.

Local setup:

- `cargo install cargo-fuzz`
- `rustup toolchain install nightly`

Run a target:

- `cargo +nightly fuzz run parser -- -max_total_time=60`
- `cargo +nightly fuzz run frontend -- -max_total_time=60`
- `cargo +nightly fuzz run runtime -- -max_total_time=60`

Corpus seeds live in `fuzz/corpus/<target>/`.

