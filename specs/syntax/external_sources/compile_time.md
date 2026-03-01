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

See the full [`@static` decorator reference](/syntax/decorators/static) for details.
