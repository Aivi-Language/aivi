# Schema-First Source Definitions

<!-- quick-info: {"kind":"topic","name":"schema-first source definitions"} -->
Schema-first source definitions put connector config and schema information on the source declaration itself, while keeping `load` as the effectful execution step.
<!-- /quick-info -->

Schema-first style is about naming an external boundary once and making its contract explicit.

Instead of only writing:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_01.aivi{aivi}


you can declare the source itself with its connector config and schema:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_02.aivi{aivi}


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

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_04.aivi{aivi}


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

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_06.aivi{aivi}


The type annotation on `usersSource` matters because it gives the compiler the result contract up front.

### Checked external contract example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_07.aivi{aivi}


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

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_08.aivi{aivi}


### REST example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_09.aivi{aivi}


### Database example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_10.aivi{aivi}


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

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_11.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_12.aivi{aivi}


### Environment

Before:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_13.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_14.aivi{aivi}


### REST / HTTP

Before:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_15.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_16.aivi{aivi}


### Database

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_17.aivi{aivi}


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
