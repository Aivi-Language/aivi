# Schema-First Source Definitions

<!-- quick-info: {"kind":"topic","name":"schema-first source definitions"} -->
Phase 3 lifts schema information out of the `load` call site and into the source declaration itself, while keeping `load` as the effectful execution step.
<!-- /quick-info -->

> [!NOTE]
> Phase 3 status:
> - This page specifies the public schema-first declaration model.
> - Existing `load (file.json "...")`, `load (rest.get ...)`, `load (env.decode ...)`, and `db.load ...` flows remain valid compatibility forms until the runtime/compiler implementation ships.
> - The preferred public story becomes: **declare the source with typed config + schema first, then `load` it**.

## Why schema-first declarations exist

Today, most source typing becomes visible only when `load` runs and the surrounding expected type drives decoding:

```aivi
do Effect {
  users : List User <- load (file.json "./users.json")
  pure users
}
```

That keeps the runtime boundary explicit, but it leaves important information fragmented:

- the connector config lives inside the constructor call,
- the schema contract is implicit in the `load` context,
- compile-time validation has no stable declaration to analyze,
- migration from one connector or schema version to another is ad hoc.

Phase 3 keeps `Source K A` and `load`, but makes the **source value** itself carry the schema contract.

## Source declaration model

A schema-first source declaration is still a pure value. Constructing it performs no I/O.
The declaration bundles three things:

1. **typed connector configuration** for the source kind,
2. a **schema strategy** describing the expected external shape,
3. optional **compile-time contract inputs** such as checked-in JSON Schema, OpenAPI, or table definitions.

Conceptually:

```aivi
-- illustrative shape; exact runtime representation is not part of the public contract
SourceDecl K A = {
  config: ConnectorConfig K A,
  schema: SourceSchema A,
  contract: Option SourceContract
}
```

`load` remains the only effectful execution step:

```aivi
load : Source K A -> Effect (SourceError K) A
```

The Phase 3 change is therefore **declarative**, not a second effect system.

## Schema strategies

Structured sources must declare how their schema is obtained. The standard strategies are:

- `source.schema.derive`  
  Derive the schema from the declaration's result type `A`.
- `source.schema.json contract`  
  Use a `JsonSchema` value as the external contract and check that it agrees with `A`.
- `source.schema.table table`  
  Reuse a `Table A` or equivalent database schema value as the authoritative row contract.
- connector-specific schema carriers may exist, but they must still be checked against the declared `A`.

`source.schema.derive` is the default for record- and type-driven decoding, but it is only valid when the declaration site has an unambiguous expected type. Top-level declarations should therefore be annotated when using derived schemas.

### Derived schema example

```aivi
User = { id: Int, name: Text, enabled: Bool }

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive
  }
```

The type annotation on `usersSource` gives the compiler the `List User` contract up front instead of waiting for a later `load`.

### Checked external contract example

```aivi
use aivi.json

User = { id: Int, name: Text, enabled: Bool }

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

Here the checked-in JSON Schema is the connector-facing contract, and the compiler validates it against `List User` when the schema artifact is available at compile time.

## Typed connector configuration

Schema-first sources do not accept untyped "options bags". Each connector family carries a connector-specific record whose fields are statically checked.

| Connector family | Required typed config surface | Schema carrier |
| --- | --- | --- |
| File / JSON / CSV | `path`, format-specific options such as delimiter/header handling, optional text encoding hints | `source.schema.derive` or `source.schema.json ...` |
| REST / HTTP | request method, `Url`, typed headers/auth fields, timeout, and status handling | derived schema or an explicit JSON/OpenAPI-derived contract |
| Environment | prefix or variable names, defaults/example values for missing fields, optional strictness flags | usually `source.schema.derive` |
| Database | connection selection, table/query/projection, typed parameters, migration/schema anchor | usually `source.schema.table ...` or a row-schema contract |

This keeps configuration validation local to the declaration:

```aivi
AppConfig = { port: Int, debug: Bool }

appConfig : Source Env AppConfig
appConfig =
  env.decode {
    prefix: "AIVI_APP",
    schema: source.schema.derive,
    strict: True
  }
```

For REST / HTTP sources:

```aivi
User = { id: Int, name: Text, enabled: Bool }

usersApi : Source RestApi (List User)
usersApi =
  rest.get {
    url: ~u(https://api.example.com/users),
    schema: source.schema.derive,
    timeoutMs: 5_000,
    strictStatus: True
  }
```

For database-backed reads, the declaration must carry either a table schema or an explicit row contract:

```aivi
usersRows : Source Db (List User)
usersRows =
  db.source {
    table: usersTable,
    schema: source.schema.table usersTable
  }
```

The exact constructor names may vary by connector, but the public contract is fixed: **typed config + schema live on the source declaration, not only at the `load` site**.
Cross-source composition concerns such as reusable retry/backoff stages, caching, and provenance belong to the separate composition spec.

## Compile-time schema validation

Compile-time validation runs when the declaration's schema inputs are compile-time stable, for example:

- literal config records,
- `@static` schema artifacts,
- `@static` OpenAPI discovery,
- checked-in database table/migration values.

When those inputs are available, the compiler must validate the declaration before code generation:

1. **schema extraction**  
   derive or load the connector-facing schema contract;
2. **schema agreement**  
   compare that contract with the declared result type `A`;
3. **connector validation**  
   validate connector-specific fields such as CSV headers, environment defaults, request options, or query parameter shapes;
4. **migration guidance**  
   if the previous checked contract and the new contract disagree, surface the mismatch at compile time with a suggested migration path.

### Validation boundaries

- Compile-time validation is **opt-in through compile-time-stable inputs**; the compiler must not perform ambient network discovery just because a source declaration exists.
- Remote contract fetching is allowed only when the user makes it explicit through `@static` (for example `openapi.fromUrl` or another explicit compile-time source).
- Successful compile-time validation never removes runtime decoding. `load` still performs the final decode and may produce `DecodeError` values when live data diverges from the contract.

### Diagnostics

Schema-first validation should report errors at the declaration site, not only inside a later `load`.

Typical diagnostics include:

- missing result type annotation for `source.schema.derive`,
- JSON Schema fields that disagree with the declared record shape,
- CSV headers that cannot populate a required field,
- environment defaults whose type does not match the target field,
- query/table projections that do not match the declared row type.

## Migration from `load`-only flows

Phase 3 is intentionally incremental. `load` stays, and most existing code can migrate mechanically.

### File / JSON

Before:

```aivi
do Effect {
  users : List User <- load (file.json "./users.json")
  pure users
}
```

After:

```aivi
usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive
  }

do Effect {
  users <- load usersSource
  pure users
}
```

### Environment decode

Before:

```aivi
do Effect {
  cfg : AppConfig <- load (env.decode "AIVI_APP")
  pure cfg
}
```

After:

```aivi
appConfig : Source Env AppConfig
appConfig =
  env.decode {
    prefix: "AIVI_APP",
    schema: source.schema.derive
  }

do Effect {
  cfg <- load appConfig
  pure cfg
}
```

### REST / HTTP

Before:

```aivi
do Effect {
  users : List User <- load (rest.get ~u(https://api.example.com/users))
  pure users
}
```

After:

```aivi
usersApi : Source RestApi (List User)
usersApi =
  rest.get {
    url: ~u(https://api.example.com/users),
    schema: source.schema.derive
  }

do Effect {
  users <- load usersApi
  pure users
}
```

### Database

Existing `db.load table` remains valid compatibility sugar for full-table reads. The schema-first model prefers naming the source declaration so compile-time validation and migration tooling can attach to it:

```aivi
usersRows : Source Db (List User)
usersRows =
  db.source {
    table: usersTable,
    schema: source.schema.table usersTable
  }
```

### Recommended migration order

1. keep the existing `load` calls,
2. extract repeated source constructors into named bindings,
3. add an explicit schema strategy to each structured source,
4. add `@static` contracts only where compile-time validation is valuable,
5. adopt migration diagnostics before replacing compatibility constructors.

Raw `Text` / `Bytes` sources such as `file.read` do not need a second schema contract; their boundary type is already the source result.

## Relationship to composition work

This page only defines how a **single source declaration** carries schema and connector information.
Higher-level composition concerns—decode/transform/validate stages, retries as reusable pipeline stages, caching, provenance, and mocking composition—belong to the separate source-composition specification.
