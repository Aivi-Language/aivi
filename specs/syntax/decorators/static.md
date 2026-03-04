# `@static` — Compile-Time Evaluation

<!-- quick-info: {"kind":"decorator","name":"@static"} -->
`@static` evaluates deterministic source reads at compile time and embeds the value into the program as a constant. No runtime overhead.
<!-- /quick-info -->

## Syntax

```aivi
@static
binding = source.call "argument"
```

## Supported Sources (v0.1)

| Source call                    | Result type       | Description                                        |
|:------------------------------ |:----------------- |:-------------------------------------------------- |
| `file.read "path"`             | `Text`            | Embed file contents as text                        |
| `file.json "path"`             | inferred from use | Parse JSON, embed as typed value                   |
| `file.csv "path"`              | `List { ... }`    | Parse CSV, embed as list of records                |
| `env.get "KEY"`                | `Text`            | Embed environment variable value                   |
| `openapi.fromUrl url`          | typed module      | Generate typed API client from an OpenAPI spec URL |
| `openapi.fromFile "path"`      | typed module      | Generate typed API client from a local spec file   |

## Examples

<<< ../../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

```aivi
@static
buildEnv = env.get "AIVI_BUILD_ENV"

@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

@static
internalApi = openapi.fromFile "./specs/internal-api.yaml"
```

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

## OpenAPI Source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) spec at compile time and generate a typed, callable API client.
<!-- /quick-info -->

The generated value is a **factory function** that takes a configuration record and returns a record of callable endpoint functions:

```aivi
@static
petStoreApi = openapi.fromFile "./petstore.json"

// Create a client with config
client = petStoreApi { bearerToken: Some "sk-...", baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }

// Call an endpoint — returns Source RestApi (Result Error Response)
pets <- client.listPets { limit: Some 10 }

// Destructuring also works
{ listPets, createPets } = petStoreApi { bearerToken: None, baseUrl: None, headers: None, timeoutMs: None, retryCount: None, strictStatus: None }
result <- listPets {}
```

**Config Record Fields:**

| Field          | Type            | Description                                          |
|:-------------- |:--------------- |:---------------------------------------------------- |
| `bearerToken`  | `Option Text`   | Bearer token for `Authorization` header              |
| `headers`      | `Option (List (Text, Text))` | Additional HTTP headers (key-value pairs) |
| `timeoutMs`    | `Option Int`    | Request timeout in milliseconds                      |
| `retryCount`   | `Option Int`    | Number of retries on failure                         |
| `strictStatus` | `Option Bool`   | Treat non-2xx responses as errors                    |
| `baseUrl`      | `Option Text`   | Override the base URL from the spec                  |

**Endpoint Parameters:**

Each endpoint function takes a record of parameters. Parameters from the OpenAPI spec (path, query, header) are mapped by name. For `POST`/`PUT`/`PATCH` endpoints, any extra fields become the JSON request body. Required parameters are direct fields; optional parameters are `Option T`.

**Type Mapping:**

| OpenAPI Type                      | AIVI Type     |
|:--------------------------------- |:------------- |
| `string`                          | `Text`        |
| `integer` / `int32` / `int64`     | `Int`         |
| `number` / `float` / `double`     | `Float`       |
| `boolean`                         | `Bool`        |
| `array` of `T`                    | `List T`      |
| `object` (with properties)        | closed record |
| `$ref`                            | named type    |
| nullable / not required           | `Option T`    |
| `oneOf` / `anyOf`                 | sum type (ADT)|
| `enum` (strings)                  | sum type      |
| `string` with `format: date`      | `Date`        |
| `string` with `format: date-time` | `DateTime`    |

Endpoint functions are named from `operationId` (lowerCamelCase); if absent, derived from method + path. Cached in `.aivi-cache/openapi/`; pass `--refresh-static` to force re-fetch. Accepts `.json`, `.yaml`, `.yml`, and Swagger 2.0 (auto-converted).
