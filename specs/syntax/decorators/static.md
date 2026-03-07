# `@static` — Compile-Time Evaluation

<!-- quick-info: {"kind":"decorator","name":"@static"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program as a constant. No runtime overhead.
<!-- /quick-info -->

Use `@static` when a value should be fetched, read, or generated during compilation instead of at runtime.
Typical uses include bundling configuration files, generating clients from schemas, and baking build-time data into the executable.

## Syntax

```aivi
@static
binding = source.call "argument"
```

The binding must be parameterless because the compiler evaluates it before the program runs.

## Supported sources (v0.1)

| Source call | Result type | Practical use |
|:----------- |:----------- |:------------- |
| `file.read "path"` | `Text` | Embed a text file directly |
| `file.json "path"` | inferred from use | Load typed JSON data at build time |
| `file.csv "path"` | `List { ... }` | Ship CSV data as records |
| `env.get "KEY"` | `Text` | Bake an environment value into the build |
| `openapi.fromUrl url` | typed module | Generate an API client from a remote OpenAPI spec |
| `openapi.fromFile "path"` | typed module | Generate an API client from a local OpenAPI spec |
| `type.jsonSchema TypeName` | `Text` | Generate OpenAI-compatible JSON Schema from a type |

## Examples

<<< ../../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

```aivi
@static
buildEnv = env.get "AIVI_BUILD_ENV"   // captured once during compilation

@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

@static
internalApi = openapi.fromFile "./specs/internal-api.yaml"
```

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
| E1518 | OpenAPI spec fetch/read failure |
| E1519 | OpenAPI spec parse failure |
| E1520 | Unsupported OpenAPI feature in type mapping |
| E1554 | `type.jsonSchema` missing or invalid type name |
| E1555 | `type.jsonSchema` type not found in module |

## OpenAPI source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) spec at compile time and generate a typed, callable API client.
<!-- /quick-info -->

The generated value is a factory function.
You pass it a configuration record, and it returns a record of endpoint functions.

```aivi
@static
petStoreApi = openapi.fromFile "./petstore.json"

client = petStoreApi {
  bearerToken: Some "sk-...",
  baseUrl: None,
  headers: None,
  timeoutMs: None,
  retryCount: None,
  strictStatus: None
}

pets <- client.listPets { limit: Some 10 }   // generated endpoint function

{ listPets, createPets } = petStoreApi {
  bearerToken: None,
  baseUrl: None,
  headers: None,
  timeoutMs: None,
  retryCount: None,
  strictStatus: None
}
result <- listPets {}
```

### Config record fields

| Field | Type | Description |
|:----- |:---- |:----------- |
| `bearerToken` | `Option Text` | Bearer token for the `Authorization` header |
| `headers` | `Option (List (Text, Text))` | Additional HTTP headers |
| `timeoutMs` | `Option Int` | Request timeout in milliseconds |
| `retryCount` | `Option Int` | Number of retries on failure |
| `strictStatus` | `Option Bool` | Treat non-2xx responses as errors |
| `baseUrl` | `Option Text` | Override the base URL from the spec |

### Endpoint parameters

Each generated endpoint function takes a record of parameters.
Path, query, and header parameters are mapped by name.
For `POST`, `PUT`, and `PATCH` endpoints, extra fields become the JSON request body.
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

Generated endpoint names come from `operationId` when present; otherwise they are derived from HTTP method and path.
OpenAPI results are cached in `.aivi-cache/openapi/`; pass `--refresh-static` to force a refresh.

## JSON Schema generation

<!-- quick-info: {"kind":"topic","name":"type.jsonSchema compile-time source"} -->
`type.jsonSchema` converts an AIVI type alias into an [OpenAI-compatible JSON Schema](https://platform.openai.com/docs/guides/structured-outputs) at compile time. The result is a `Text` value containing the JSON schema string.
<!-- /quick-info -->

Use this when an external system, such as an LLM API, expects a JSON Schema description of structured output.

### Syntax

```aivi
@static
schemaBinding = type.jsonSchema TypeName
```

`TypeName` must be a type alias defined in the same module.
The generated schema is wrapped in the OpenAI structured-output envelope:

```json
{
  "format": {
    "type": "json_schema",
    "name": "TypeName",
    "schema": { ... },
    "strict": true
  }
}
```

### Example

```aivi
ExtractionResult = {
  title: Text,
  summary: Text,
  tags: List Text,
  score: Option Float
}

@static
extractionSchema = type.jsonSchema ExtractionResult   // becomes a compile-time `Text` constant
```

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

Records set `"additionalProperties": false` and include all non-optional fields in `"required"`.
