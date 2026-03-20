# Compile-Time Sources (`@static`)

<!-- quick-info: {"kind":"topic","name":"compile-time sources"} -->
`@static` evaluates deterministic source reads and build-time generation steps at compile time and embeds the value into the program.
<!-- /quick-info -->

`@static` is for inputs that should be fixed when the program is built rather than read every time the program runs.
This page focuses on when to use compile-time sources as part of source design. For decorator syntax and full feature details, see [`@static`](../decorators/static.md).
Like all `@static` bindings, these definitions must be parameterless.

Common uses:

- checked-in JSON or CSV files,
- build metadata from environment variables,
- schema artifacts used for validation,
- generated clients or schema contracts discovered explicitly at compile time.

In practical terms, `@static` fixes data or generated artifacts at build time. That gives you simpler deployment and earlier failures, but it also means the running program will not see later changes unless you rebuild.

## Supported patterns

- `@static x = file.read "..."`
- `@static x = file.json "..."`
- `@static x = file.csv "..."`
- `@static x = env.get "..."`
- `@static x = openapi.fromUrl ~url(...)` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)
- `@static x = openapi.fromFile "..."` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)
- `@static x = type.jsonSchema TypeName` — see [JSON Schema generation](/syntax/decorators/static#json-schema-generation)

## Basic examples

<<< ../../snippets/from_md/syntax/external_sources/compile_time/block_01.aivi{aivi}


Use this style when you want the compiled binary to carry the value directly.
For file-based reads, relative paths resolve from the source file first and then from the workspace root.

You can also generate contracts at compile time:

<<< ../../snippets/from_md/syntax/decorators/static/block_05.aivi{aivi}

`type.jsonSchema` is useful when another build-time tool expects a JSON Schema contract derived from an AIVI type alias in the same module.

## What happens on failure

Compilation fails if a static source cannot be read, fetched, decoded, or generated.

That makes `@static` a good fit for inputs that must be present and valid before you ship:

- missing files fail the build,
- invalid JSON or CSV fails the build,
- unreadable or invalid OpenAPI specs fail the build,
- invalid `type.jsonSchema` inputs fail the build.

## Using `@static` with schema validation

One of the most useful patterns is to load a schema artifact at compile time and then attach it to a runtime source declaration.

<<< ../../snippets/from_md/syntax/external_sources/compile_time/block_02.aivi{aivi}


When a source declaration uses compile-time-stable config and schema inputs, the compiler can validate the declaration before code generation. In practice, that means:

- loading the schema artifact,
- checking that the schema agrees with the declared result type,
- reporting mismatches at the declaration site.

Runtime decoding still happens when you later call `load usersSource`. Compile-time validation catches bad contracts early; it does not remove the runtime boundary.

`type.jsonSchema` follows the same build-time pattern when you need to derive a contract from an AIVI type first and hand that generated schema to another tool.

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

See [Environment Sources](environment.md) for the runtime version of environment lookups, [Schema-First Source Definitions](schema_first.md) for how compile-time artifacts fit into source declarations, and the full [`@static` decorator reference](/syntax/decorators/static#static-compile-time-evaluation) for details.
