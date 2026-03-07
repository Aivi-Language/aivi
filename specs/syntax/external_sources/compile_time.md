# Compile-Time Sources (`@static`)

<!-- quick-info: {"kind":"topic","name":"compile-time sources"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program.
<!-- /quick-info -->

`@static` is for inputs that should be fixed when the program is built rather than read every time the program runs.
This page focuses on when to use compile-time sources as part of source design. For decorator syntax and full feature details, see [`@static`](../decorators/static.md).

Common uses:

- checked-in JSON or CSV files,
- build metadata from environment variables,
- schema artifacts used for validation,
- generated clients or contracts discovered explicitly at compile time.

In practical terms, `@static` fixes data at build time. That gives you simpler deployment and earlier failures, but it also means the running program will not see later changes unless you rebuild.

## Supported patterns

- `@static x = file.read "..."`
- `@static x = file.json "..."`
- `@static x = file.csv "..."`
- `@static x = env.get "..."`
- `@static x = openapi.fromUrl ~url(...)` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)
- `@static x = openapi.fromFile "..."` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)

## Basic examples

```aivi
@static
schema = file.json "./schema.json"        // bundled into the compiled program

@static
buildEnv = env.get "AIVI_BUILD_ENV"       // resolved when compiling, not at runtime
```

Use this style when you want the compiled binary to carry the value directly.

## What happens on failure

Compilation fails if a static source cannot be read or decoded.

That makes `@static` a good fit for inputs that must be present and valid before you ship:

- missing files fail the build,
- invalid JSON or CSV fails the build,
- missing build-time environment variables fail the build.

## Using `@static` with schema validation

One of the most useful patterns is to load a schema artifact at compile time and then attach it to a runtime source declaration.

```aivi
use aivi.json

@static
userSchema : JsonSchema
userSchema = file.json "./schemas/users.schema.json"  // checked while compiling

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.json userSchema              // runtime reads must match this contract
  }
```

When a source declaration uses compile-time-stable config and schema inputs, the compiler can validate the declaration before code generation. In practice, that means:

- loading the schema artifact,
- checking that the schema agrees with the declared result type,
- reporting mismatches at the declaration site.

Runtime decoding still happens when you later call `load usersSource`. Compile-time validation catches bad contracts early; it does not remove the runtime boundary.

## Explicit remote discovery

Compile-time validation does not imply automatic network access.

If you want to fetch a remote contract while compiling, make that choice explicit with a compile-time source such as `openapi.fromUrl`. This keeps builds predictable and makes the dependency visible in the source code.

## When to choose `@static`

Choose `@static` when the answer to these questions is "yes":

1. should this value be fixed at build time?
2. would it be better to fail during compilation than at runtime?
3. is the source deterministic enough to make builds reproducible?

If the value is expected to change between runs, keep it as a regular runtime source instead.

## When not to choose `@static`

Keep a normal runtime source when:

- the value must be fresh on every run,
- different deployments or users should see different data without rebuilding,
- the source is non-deterministic enough to make builds hard to reproduce.

See [Schema-First Source Definitions](schema_first.md) for how compile-time artifacts fit into source declarations, and see the full [`@static` decorator reference](/syntax/decorators/static#static-compile-time-evaluation) for details.
