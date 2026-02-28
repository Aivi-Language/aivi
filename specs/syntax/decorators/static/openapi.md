# OpenAPI Compile-Time Source

<!-- quick-info: {"kind":"topic","name":"openapi compile-time source"} -->
`openapi.fromUrl` and `openapi.fromFile` are `@static` sources that parse an [OpenAPI 3.x](https://spec.openapis.org/oas/v3.1.1.html) specification at compile time and generate a fully typed AIVI module with functions for each endpoint.
<!-- /quick-info -->

## Syntax

```aivi
// From a remote URL (fetched at compile time)
@static
petStore = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

// From a local file (JSON or YAML)
@static
internalApi = openapi.fromFile "./specs/internal-api.yaml"
```

The result is a **record** whose fields are typed functions — one per endpoint.

## Generated Module Shape

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

## Type Mapping

OpenAPI types map to AIVI types as follows:

| OpenAPI Type              | AIVI Type        |
|:------------------------- |:---------------- |
| `string`                  | `Text`           |
| `integer` / `int32`       | `Int`            |
| `integer` / `int64`       | `Int`            |
| `number` / `float`        | `Float`          |
| `number` / `double`       | `Float`          |
| `boolean`                 | `Bool`           |
| `array` of `T`            | `List T`         |
| `object` (with properties)| closed record    |
| `$ref`                    | named type       |
| nullable / not required   | `Option T`       |
| `oneOf` / `anyOf`         | sum type (ADT)   |
| `enum` (strings)          | sum type          |
| `string` with `format: date` | `Date`        |
| `string` with `format: date-time` | `DateTime` |

### Required vs Optional Fields

- Properties listed in `required` are mapped directly to their AIVI type.
- Properties not in `required` are wrapped in `Option`.
- Nullable properties are always `Option`.

### Sum Types from `oneOf`

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

### String Enums

```yaml
Status:
  type: string
  enum: [active, inactive, pending]
```

Generates:

```aivi
Status = Active | Inactive | Pending
```

## Endpoint Function Naming

- **`operationId`** is used as the function name (converted to `lowerCamelCase`).
- If no `operationId` is present, a name is derived from the HTTP method and path: `GET /pets/{petId}` → `getPetsPetId`.

## Parameter Handling

| Parameter location | Mapping                                                    |
|:------------------ |:---------------------------------------------------------- |
| `path`             | Positional function argument (required)                    |
| `query`            | Record field in an options argument (`Option` if optional) |
| `header`           | Record field in an options argument                        |
| `requestBody`      | Dedicated typed argument                                   |

When an endpoint has only path parameters and no optional query/header params, the function takes simple positional arguments. When query/header parameters are present, they are grouped into an options record.

## Authentication

If the spec defines `securitySchemes`, the generated module includes a config record:

```aivi
// If the spec uses Bearer auth
petStore.withAuth : { bearerToken: Text } -> petStore
```

This returns a copy of the API module with the auth header injected into all requests.

## Error Handling

All generated functions return `Effect (SourceError RestApi) A`. HTTP errors (4xx, 5xx) surface as `SourceError` values with status code, message, and response body.

## Supported OpenAPI Versions

- OpenAPI 3.0.x — fully supported
- OpenAPI 3.1.x — fully supported
- OpenAPI/Swagger 2.0 — supported (auto-converted internally)

## Compile-Time Errors

| Code  | Condition                                       |
|:----- |:----------------------------------------------- |
| E1518 | URL fetch failure or file read failure           |
| E1519 | Spec parse failure (invalid JSON/YAML or schema) |
| E1520 | Unsupported OpenAPI feature in type mapping      |

## Caching

When using `openapi.fromUrl`, the fetched spec is cached in `.aivi-cache/openapi/` keyed by URL hash. Subsequent compilations reuse the cache unless `--refresh-static` is passed.

## Example: Full Usage

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
