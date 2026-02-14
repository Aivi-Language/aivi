# Golden Tests

This directory holds deterministic "golden" (snapshot) tests for the AIVI toolchain.

Layout:

- `cases/<name>/input.aivi`: the input program
- `cases/<name>/parse.cst.json`: snapshot of `aivi::parse_target` (CST + lexer/parser diagnostics)
- `cases/<name>/check.diagnostics.json`: snapshot of resolver + typechecker diagnostics
- `cases/<name>/fmt.aivi`: snapshot of `aivi fmt` output (also checks idempotence)

Updating snapshots locally:

- `AIVI_BLESS=1 cargo test -p aivi --test golden_harness`

CI never sets `AIVI_BLESS`, so snapshots must be updated deliberately.

