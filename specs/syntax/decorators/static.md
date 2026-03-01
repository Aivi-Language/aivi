# `@static` — Compile-Time Evaluation

<!-- quick-info: {"kind":"decorator","name":"@static"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program as a constant. No runtime overhead.
<!-- /quick-info -->

## Syntax

```aivi
@static
binding = source.call "argument"
```

The decorator appears on the line above the binding. The right-hand side must be a supported compile-time source call.

## Supported Sources (v0.1)

| Source call            | Result type       | Description                          |
|:---------------------- |:----------------- |:------------------------------------ |
| `file.read "path"`    | `Text`            | Embed file contents as text          |
| `file.json "path"`    | inferred from use | Parse JSON, embed as typed value     |
| `file.csv "path"`     | `List { ... }`    | Parse CSV, embed as list of records  |
| `env.get "KEY"`       | `Text`            | Embed environment variable value     |
| `openapi.fromUrl url` | typed module      | Generate typed API client from an OpenAPI spec URL |
| `openapi.fromFile "path"` | typed module  | Generate typed API client from a local OpenAPI spec file |

## Examples

### File Embedding

<<< ../../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

The compiler evaluates the right-hand side at compile time and embeds the result as a constant.

### Environment Variables

```aivi
@static
buildEnv = env.get "AIVI_BUILD_ENV"
```

### OpenAPI Client Generation

```aivi
@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

@static
internalApi = openapi.fromFile "./specs/internal-api.yaml"
```

The compiler fetches (or reads) the OpenAPI specification at compile time, parses it, and generates a typed record with functions for each endpoint. See [OpenAPI Source](./static/openapi) for the full type-mapping and generated module shape.

## Semantics

- Compilation **fails early** if a static source cannot be read, fetched, or decoded.
- The embedded value is a **constant** — no I/O happens at runtime.
- File paths are resolved relative to the source file first, then the workspace root.
- `@static` bindings must be **parameterless** (no function parameters).

## Compile-Time Errors

| Code  | Condition                                      |
|:----- |:---------------------------------------------- |
| E1514 | `@static` applied to a parameterised binding   |
| E1515 | File read failure                              |
| E1516 | JSON parse failure                             |
| E1517 | CSV parse failure                              |
| E1518 | OpenAPI spec fetch/read failure                |
| E1519 | OpenAPI spec parse failure (invalid schema)    |
| E1520 | Unsupported OpenAPI feature in type mapping    |
