# Performance Regression Tests

This directory contains deterministic fixtures and a small benchmark harness.

Fixtures:

- `fixtures/*.aivi`: representative input programs (kept small and hermetic)

Baseline:

- `baseline.json`: the expected medians on CI runners (used as a reference point)

Run locally:

- `cargo run -p aivi --bin perf -- run`
- `cargo run -p aivi --bin perf -- check --baseline crates/aivi/perf/baseline.json --max-multiplier 2.0`

Notes:

- Perf checks are intentionally lenient on PRs (multiplier 2.0) to avoid flakiness.
- Nightly uses a tighter multiplier.

