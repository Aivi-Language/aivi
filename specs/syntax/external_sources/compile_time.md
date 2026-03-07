# Compile-Time Sources (`@static`)

<!-- quick-info: {"kind":"topic","name":"compile-time sources"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program.
<!-- /quick-info -->

## Supported patterns (v0.1)

- `@static x = file.read "..."`
- `@static x = file.json "..."`
- `@static x = file.csv "..."`
- `@static x = env.get "..."`
- `@static x = openapi.fromUrl ~url(...)` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)
- `@static x = openapi.fromFile "..."` — see [OpenAPI Source](/syntax/decorators/static#openapi-source)

## Example

```aivi
@static
schema = file.json "./schema.json"

@static
buildEnv = env.get "AIVI_BUILD_ENV"
```

Compilation fails early if a static source cannot be read or decoded.

## Schema-first validation

Phase 3 uses `@static` as the explicit path for compile-time schema validation of source declarations.

```aivi
use aivi.json

@static
userSchema : JsonSchema
userSchema = file.json "./schemas/users.schema.json"

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.json userSchema
  }
```

When a source declaration's config record and schema artifacts are compile-time stable, the compiler must:

- load and validate the schema artifact,
- compare it with the declaration's result type,
- surface mismatches at the declaration site before code generation.

Compile-time schema validation does **not** introduce implicit remote discovery. Any remote schema fetch must itself be an explicit `@static` source such as `openapi.fromUrl`.

See [Schema-First Source Definitions](schema_first.md) for the full Phase 3 model.

See the full [`@static` decorator reference](/syntax/decorators/static#static-compile-time-evaluation) for details.
