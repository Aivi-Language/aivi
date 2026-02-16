# Missing Features (v0.1)

This document tracks features specified in the documentation but not yet implemented (or only partially implemented) in the v0.1 codebase.

## Test Coverage (v0.1)

### Strong Coverage
- **Parser & AST**: Extensive unit tests in `crates/aivi/src/surface/tests.rs` and `crates/aivi/tests/parse_golden.rs`.
- **Type Checker**: Integration tests in `crates/aivi/tests/typecheck_core.rs` covering effects, patching, domains, and type classes.
- **LSP Server**: Comprehensive functional tests in `crates/aivi_lsp/src/tests.rs` covering completion, hover, definition, and diagnostics.

### Moderate Coverage
- **Runtime**: Smoke tests in `crates/aivi/tests/runtime_smoke.rs` run key integration tests to verify end-to-end execution.
- **VSCode Extension**: `vitest` configuration exists, but specific test files need verification (pending scan of `src/test/`).

### Missing / Weak Coverage
- **Standard Library Unit Tests**: No dedicated unit test suite for stdlib modules (e.g. `math`, `collections`, `text`). Reliance is placed on integration smoke tests.
- **Native Runtime**: `aivi_native_runtime` is a dependency, but its specific test suite coverage is unverified in this pass.

## Other Observations
- **Database**: `aivi.database` is implemented with a driver abstraction, but specific driver implementations (SQLite, Postgres, MySQL) rely on the runtime environment configuration.
- **HTTP Server**: fully implemented (`aivi.net.http_server`).
- **External Sources**: `Source`/`load` exist, but structured decoders (`file.json`/`file.csv`), streaming sources, and `env.decode` are not implemented in v0.1.
