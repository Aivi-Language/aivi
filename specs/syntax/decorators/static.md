# `@static` — Compile-Time Evaluation

<!-- quick-info: {"kind":"decorator","name":"@static"} -->
`@static` evaluates deterministic source reads (reads whose inputs are fully known at compile time, such as checked-in files or build metadata) at compile time and embeds the resulting value into the program as a constant. The running program does not re-read, re-fetch, or re-decode that binding.
<!-- /quick-info -->

Use `@static` when a value should be fetched, read, or generated during compilation instead of at runtime.
Typical uses include bundling configuration files, generating clients from schemas, and baking build-time data into the executable.
If you want design guidance rather than decorator syntax, see [Compile-Time Sources](../external_sources/compile_time.md).

## Syntax

<<< ../../snippets/from_md/syntax/decorators/static/block_01.aivi{aivi}


The binding must be parameterless because the compiler evaluates it before the program runs and there are no runtime arguments to supply.

## Start with the common cases

Most people reach for `@static` in one of three situations:

1. **embed checked-in data** such as JSON, CSV, or text files,
2. **capture build-time environment values** once during compilation,
3. **generate helper artifacts** such as API clients or JSON Schema text.

## Supported sources (v0.1)

| Source call | Embedded value | Practical use |
|:----------- |:------------- |:------------- |
| `file.read "path"` | `Text` | Embed a text file directly |
| `file.json "path"` | value inferred from the JSON shape | Load typed JSON data at build time |
| `file.csv "path"` | `List { ... }` | Ship CSV data as records |
| `env.get "KEY"` | `Text` | Bake an environment value into the build |
| `openapi.fromUrl ~url(...)` | factory function `Config -> { endpoints... }` | Generate an API client from a remote OpenAPI spec |
| `openapi.fromFile "path"` | factory function `Config -> { endpoints... }` | Generate an API client from a local OpenAPI spec |
| `type.jsonSchema TypeName` | compile-time schema value (render with `toText` when you need JSON text) | Generate OpenAI-compatible JSON Schema from a type |

If you are just starting, focus on the first four rows. The OpenAPI and `type.jsonSchema` entries are the more advanced build-time code-generation cases.

## Examples

The snippets below move from simple file embedding to the more advanced code-generation cases. The OpenAPI and `type.jsonSchema` examples are mirrored by `integration-tests/syntax/decorators/static_openapi.aivi` and `integration-tests/syntax/decorators/static_json_schema.aivi`.

<<< ../../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

<<< ../../snippets/from_md/syntax/decorators/static/block_02.aivi{aivi}


## Semantics

- Compilation fails early if the static source cannot be read, fetched, or decoded.
- The embedded value is a constant, so the running program performs no I/O for that binding.
- File paths are resolved relative to the source file first, then the workspace root.
- `@static` bindings must be parameterless.

## Compile-time errors

| Code | Condition |
|:---- |:--------- |
| E1514 | `@static` applied to a parameterised binding |
| E1515 | File read failure |
| E1516 | JSON parse failure |
| E1517 | CSV parse failure |
| E1518 | OpenAPI source argument, fetch, or file-read failure |
| E1519 | OpenAPI spec parse failure |
| E1554 | `type.jsonSchema` missing or invalid type name |
| E1555 | `type.jsonSchema` type not found in module |

## OpenAPI source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) spec at compile time and generate a typed, callable API client.
<!-- /quick-info -->

The generated value is a factory function.
You pass it a configuration record, and it returns a record of endpoint functions.

<<< ../../snippets/from_md/syntax/decorators/static/block_03.aivi{aivi}


### Config record fields

| Field | Type | Description |
|:----- |:---- |:----------- |
| `bearerToken` | `Option Text` | Bearer token for the `Authorization` header |
| `headers` | `Option (List (Text, Text))` | Additional HTTP headers |
| `timeoutMs` | `Option Int` | Request timeout in milliseconds |
| `retryCount` | `Option Int` | Number of retries on failure |
| `strictStatus` | `Option Bool` | Treat non-2xx responses as errors |
| `baseUrl` | `Option Text` | Override the base URL from the spec |

Every field is optional, so calling the generated factory with `{}` uses the defaults.

### Endpoint parameters

Each generated endpoint function takes a record of parameters.
Path, query, and header parameters are mapped by name.
For `POST`, `PUT`, and `PATCH` endpoints, fields not consumed as path/query/header parameters become the JSON request body.
Required parameters stay direct fields; optional parameters become `Option T`.

### Type mapping

| OpenAPI type | AIVI type |
|:------------ |:--------- |
| `string` | `Text` |
| `integer` / `int32` / `int64` | `Int` |
| `number` / `float` / `double` | `Float` |
| `boolean` | `Bool` |
| `array` of `T` | `List T` |
| `object` with properties | closed record |
| `$ref` | named type |
| nullable or not required | `Option T` |
| `oneOf` / `anyOf` | sum type (ADT) |
| string `enum` | sum type |
| `string` with `format: date` | `Date` |
| `string` with `format: date-time` | `DateTime` |

Generated endpoint names come from `operationId` when present; otherwise they are derived from the lowercased HTTP method plus capitalized path segments. For example, `GET /pets/{petId}` becomes `getPetsPetId`.
Rebuild the program when the OpenAPI spec changes; the generated client is refreshed during compilation, not at runtime.

## JSON Schema generation

<!-- quick-info: {"kind":"topic","name":"type.jsonSchema compile-time source"} -->
`type.jsonSchema` converts an AIVI type alias into an [OpenAI-compatible JSON Schema](https://platform.openai.com/docs/guides/structured-outputs) envelope at compile time. Render the embedded value with `toText` when you need the JSON document itself.
<!-- /quick-info -->

Use this when an external system, such as an LLM API or validation service, expects a JSON Schema description of structured output and you want that schema generated during the build.

### Syntax

<<< ../../snippets/from_md/syntax/decorators/static/block_02.aivi{aivi}


`TypeName` must be a type alias defined in the same module.
The generated schema follows the OpenAI structured-output envelope:

```json
{
  "type": "json_schema",
  "json_schema": {
    "name": "TypeName",
    "schema": { ... },
    "strict": true
  }
}
```

### Example

The example below shows the compile-time binding and a typical `toText` rendering step for handing the schema to another system.

<<< ../../snippets/from_md/syntax/decorators/static/block_05.aivi{aivi}


### Type mapping

| AIVI type | JSON Schema |
|:--------- |:----------- |
| `Text` | `{"type": "string"}` |
| `Int` | `{"type": "integer"}` |
| `Float` | `{"type": "number"}` |
| `Bool` | `{"type": "boolean"}` |
| `List T` | `{"type": "array", "items": ...}` |
| `{ field: T, ... }` | `{"type": "object", "properties": ...}` |
| `Option T` | inner schema with `"nullable": true` |
| `(A, B, C)` | `{"type": "array", "prefixItems": [...]}` |
| ADT with only named cases | `{"type": "string", "enum": [...]}` |
| mixed-form ADT | `{"anyOf": [...]}` |
| unresolved or function type | `{"type": "string"}` fallback |

Records set `"additionalProperties": false`. In the current strict structured-output envelope, every declared field appears in `"required"`; `Option T` fields are marked with `"nullable": true` rather than being omitted from `"required"`.
