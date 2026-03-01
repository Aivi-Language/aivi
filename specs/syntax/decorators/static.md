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

The compiler fetches (or reads) the OpenAPI specification at compile time, parses it, and generates a typed record with functions for each endpoint. See [OpenAPI Source](#openapi-source) below for the full type-mapping and generated module shape.

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

---

## OpenAPI Source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` are `@static` sources that parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) specification at compile time and generate a fully typed AIVI module with functions for each endpoint.
<!-- /quick-info -->

### Generated Module Shape

Given an OpenAPI spec with endpoints like:

```yaml
paths:
  /pets:
    get:
      operationId: listPets
      parameters:
        - name: limit
          in: query
          schema: { type: integer }
      responses:
        '200':
          content:
            application/json:
              schema:
                type: array
                items: { $ref: '#/components/schemas/Pet' }
    post:
      operationId: createPet
      requestBody:
        content:
          application/json:
            schema: { $ref: '#/components/schemas/NewPet' }
      responses:
        '201':
          content:
            application/json:
              schema: { $ref: '#/components/schemas/Pet' }
  /pets/{petId}:
    get:
      operationId: getPetById
      parameters:
        - name: petId
          in: path
          schema: { type: integer }
      responses:
        '200':
          content:
            application/json:
              schema: { $ref: '#/components/schemas/Pet' }
components:
  schemas:
    Pet:
      type: object
      required: [id, name]
      properties:
        id:   { type: integer }
        name: { type: string }
        tag:  { type: string }
    NewPet:
      type: object
      required: [name]
      properties:
        name: { type: string }
        tag:  { type: string }
```

The compiler generates:

```aivi
// Auto-generated types from components/schemas
Pet = { id: Int, name: Text, tag: Option Text }
NewPet = { name: Text, tag: Option Text }

// Auto-generated endpoint functions
// Each returns Effect (SourceError RestApi) A
petStore = {
  listPets   : { limit: Option Int } -> Effect (SourceError RestApi) (List Pet)
  createPet  : NewPet -> Effect (SourceError RestApi) Pet
  getPetById : Int -> Effect (SourceError RestApi) Pet
}
```

### Type Mapping

| OpenAPI Type                      | AIVI Type          |
|:--------------------------------- |:------------------ |
| `string`                          | `Text`             |
| `integer` / `int32`               | `Int`              |
| `integer` / `int64`               | `Int`              |
| `number` / `float`                | `Float`            |
| `number` / `double`               | `Float`            |
| `boolean`                         | `Bool`             |
| `array` of `T`                    | `List T`           |
| `object` (with properties)        | closed record      |
| `$ref`                            | named type         |
| nullable / not required           | `Option T`         |
| `oneOf` / `anyOf`                 | sum type (ADT)     |
| `enum` (strings)                  | sum type           |
| `string` with `format: date`      | `Date`             |
| `string` with `format: date-time` | `DateTime`         |

Properties listed in `required` map directly to their type; others are wrapped in `Option`. Nullable properties are always `Option`.

#### Sum Types from `oneOf`

```yaml
Shape:
  oneOf:
    - $ref: '#/components/schemas/Circle'
    - $ref: '#/components/schemas/Square'
```

Generates:

```aivi
Shape = Circle Circle | Square Square
```

#### String Enums

```yaml
Status:
  type: string
  enum: [active, inactive, pending]
```

Generates:

```aivi
Status = Active | Inactive | Pending
```

### Endpoint Function Naming

- **`operationId`** is used as the function name (converted to `lowerCamelCase`).
- If no `operationId` is present, a name is derived from the HTTP method and path: `GET /pets/{petId}` → `getPetsPetId`.

### Parameter Handling

| Parameter location | Mapping                                                    |
|:------------------ |:---------------------------------------------------------- |
| `path`             | Positional function argument (required)                    |
| `query`            | Record field in an options argument (`Option` if optional) |
| `header`           | Record field in an options argument                        |
| `requestBody`      | Dedicated typed argument                                   |

### Authentication

If the spec defines `securitySchemes`, the generated module includes a config record:

```aivi
// If the spec uses Bearer auth
petStore.withAuth : { bearerToken: Text } -> petStore
```

### Caching

When using `openapi.fromUrl`, the fetched spec is cached in `.aivi-cache/openapi/` keyed by URL hash. Reuse the cache on subsequent compilations; pass `--refresh-static` to force a re-fetch.

### Supported OpenAPI Versions

- OpenAPI 3.0.x — fully supported
- OpenAPI 3.1.x — fully supported
- OpenAPI/Swagger 2.0 — supported (auto-converted internally)

### Full Example

```aivi
@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

main = do Effect {
  pets <- petStore.listPets { limit: Some 10 }
  pets |> List.forEach pet =>
    console.log "Pet: {pet.name}"

  newPet <- petStore.createPet { name: "Fido", tag: Some "dog" }
  console.log "Created: {newPet.name} (id: {newPet.id})"
}
```
