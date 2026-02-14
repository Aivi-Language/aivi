# Testing AIVI

This repo uses a tiered, hermetic test strategy:

- Unit + integration tests: `cargo test --workspace`
- Golden (snapshot) tests: `cargo test -p aivi --test golden_harness`
- Fuzz tests (libFuzzer via `cargo-fuzz`): `cargo +nightly fuzz ...`
- Perf regression smoke tests: `cargo run -p aivi --bin perf -- check ...`

## Unit + Integration

Run everything:

```bash
cargo test --workspace
```

## Golden Tests

Run:

```bash
cargo test -p aivi --test golden_harness
```

Update snapshots deliberately:

```bash
AIVI_BLESS=1 cargo test -p aivi --test golden_harness
```

Golden cases live under `crates/aivi/tests/goldens/cases/*/`.

## Fuzz Tests

One-time setup:

```bash
cargo install cargo-fuzz
rustup toolchain install nightly
```

Short run (local smoke):

```bash
cargo +nightly fuzz run parser -- -max_total_time=60
cargo +nightly fuzz run frontend -- -max_total_time=60
cargo +nightly fuzz run runtime -- -max_total_time=60
```

Corpus seeds live in `fuzz/corpus/<target>/`.

## Performance Regression Tests

Run and print metrics:

```bash
cargo run -p aivi --bin perf -- run
```

Check against the repository baseline with a multiplier (lenient example):

```bash
cargo run -p aivi --bin perf -- check --baseline crates/aivi/perf/baseline.json --max-multiplier 2.0
```

