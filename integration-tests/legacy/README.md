Legacy AIVI examples

These files are older examples that were moved from `examples/` during the
integration test suite migration.

- They are not required to contain `@test` functions.
- `aivi test` only formats/parses/typechecks files that contain `@test`, so
  these files are ignored when running `aivi test integration-tests/**`.

If you want to turn any of these into runnable integration tests, add one or
more `@test`-decorated top-level definitions and then include the file under
the `aivi test` target.
