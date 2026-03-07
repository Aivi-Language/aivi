# Schema-First Source Definitions

<!-- quick-info: {"kind":"topic","name":"schema-first source definitions"} -->
Schema-first source definitions put connector config and schema information on the source declaration itself, while keeping `load` as the effectful execution step.
<!-- /quick-info -->

Schema-first style is about naming an external boundary once and making its contract explicit.

Instead of only writing:

```aivi
do Effect {
  users : List User <- load (file.json "./users.json")
  pure users
}
```

you can declare the source itself with its connector config and schema:

```aivi
usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive
  }
```

Then you can reuse it anywhere:

```aivi
do Effect {
  users <- load usersSource
  pure users
}
```

For many small programs, the shorter inline `load (...)` style is perfectly fine. Schema-first declarations become especially useful when you want reuse, explicit contracts, or compile-time validation.

## What a schema-first source declaration contains

A schema-first source is still a pure value. Building it performs no I/O.

Conceptually, the declaration bundles:

1. typed connector configuration,
2. a schema strategy that explains the expected external shape,
3. optional compile-time contract artifacts such as JSON Schema or OpenAPI data.

Illustratively:

```aivi
// illustrative shape; exact runtime representation is not part of the public contract
SourceDecl K A = {
  config: ConnectorConfig K A,
  schema: SourceSchema A,
  contract: Option SourceContract
}
```

`load` remains the only effectful step:

```aivi
load : Source K A -> Effect (SourceError K) A
```

## Schema strategies

Structured sources need a clear answer to the question "how do we know what shape to decode?"

The standard strategies are:

- `source.schema.derive`
  - derive the schema from the declaration's result type `A`
- `source.schema.json contract`
  - use a `JsonSchema` value as the external contract and check that it agrees with `A`
- `source.schema.table table`
  - reuse a `Table A` or similar database schema value as the row contract

Connector-specific schema carriers may exist, but they must still agree with the declared result type.

### Derived schema example

```aivi
User = { id: Int, name: Text, enabled: Bool }

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive   // the List User type drives the contract
  }
```

The type annotation on `usersSource` matters because it gives the compiler the result contract up front.

### Checked external contract example

```aivi
use aivi.json

User = { id: Int, name: Text, enabled: Bool }

@static
userSchema : JsonSchema
userSchema = file.json "./schemas/users.schema.json"  // load the contract while compiling

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.json userSchema              // require agreement with the declared type
  }
```

This pattern is useful when the external contract already exists outside your AIVI codebase and you want the compiler to check it early.

## Typed connector configuration

Schema-first sources use typed connector records rather than unstructured option bags.

| Connector family | Typical typed config | Schema carrier |
| --- | --- | --- |
| File / JSON / CSV | `path`, format-specific options, optional encoding hints | `source.schema.derive` or `source.schema.json ...` |
| REST / HTTP | method, `Url`, typed headers and auth fields, timeout, status handling | derived schema or an explicit JSON/OpenAPI contract |
| Environment | prefix or variable names, defaults or examples, strictness flags | usually `source.schema.derive` |
| Database | connection selection, table/query/projection, typed parameters, migration anchor | usually `source.schema.table ...` |

That makes connector validation local and readable.

### Environment example

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

### REST example

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

### Database example

```aivi
usersRows : Source Db (List User)
usersRows =
  db.source {
    table: usersTable,
    schema: source.schema.table usersTable
  }
```

The exact constructor names may vary by connector, but the public idea is the same: the source declaration carries both the connector config and the schema contract.

## Compile-time validation

Compile-time validation is most useful when the declaration's schema inputs are stable at build time, for example:

- literal config records,
- `@static` schema artifacts,
- `@static` OpenAPI discovery,
- checked-in table or migration definitions.

When those inputs are available, the compiler can validate the declaration before code generation:

1. derive or load the connector-facing contract,
2. compare that contract with the declared result type,
3. validate connector-specific fields,
4. report mismatches at the declaration site.

Typical diagnostics include:

- missing type information for `source.schema.derive`,
- JSON Schema fields that disagree with the declared record shape,
- CSV headers that cannot populate a required field,
- environment defaults with the wrong type,
- query or projection shapes that do not match the declared row type.

Compile-time validation does not remove runtime decoding. Live data can still diverge, so `load` still performs the final decode.

## Explicit remote discovery

Remote discovery must stay explicit.

If you want to fetch an OpenAPI document or another remote contract while compiling, do it through an explicit compile-time source such as `openapi.fromUrl`. The compiler should not perform surprise network discovery just because a source declaration exists.

## Moving from inline `load` calls to named declarations

The migration is usually mechanical.

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

### Environment

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

```aivi
usersRows : Source Db (List User)
usersRows =
  db.source {
    table: usersTable,
    schema: source.schema.table usersTable
  }
```

## How this relates to source composition

Schema-first declarations answer "what is this source and what contract does it promise?"

Source composition answers "what extra processing or execution policy should happen when we load it?"

Use schema-first declarations for:

- named, reusable external boundaries,
- explicit connector config,
- compile-time contract checking.

Use source composition for:

- transforms,
- semantic validation,
- retry and timeout policy,
- caching,
- provenance and observation.

See [Source Composition](composition.md) for those execution-stage details.
