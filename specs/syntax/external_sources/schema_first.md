# Schema-First Source Definitions

<!-- quick-info: {"kind":"topic","name":"schema-first source definitions"} -->
Schema-first source definitions put connector config and schema information on the source declaration itself, while keeping `load` as the effectful execution step.
<!-- /quick-info -->

If you are new to external sources, the mental model is simple: name the boundary once, keep it pure, and call `load` later where I/O should happen.

Instead of repeating the full source declaration inline at every load site:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_01.aivi{aivi}


you can name that boundary once and give it an explicit result type:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_02.aivi{aivi}


Then you can reuse it anywhere:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_01.aivi{aivi}


For many small programs, the shorter inline `load (...)` style is perfectly fine. Schema-first declarations become useful when a boundary is reused, when you want hover and diagnostics to describe it before `load`, or when you want the connector config to live in one obvious place.

## What a schema-first source declaration contains

A schema-first source is still a pure value. Building it performs no I/O; only `load` touches the outside world.

At a high level, the declaration bundles:

1. typed connector configuration,
2. a decode contract for the eventual `A`,
3. any compile-time-stable helper inputs that the connector docs say may participate in checking.

The exact internal representation is not part of the public API. The sketch below is conceptual only:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_04.aivi{aivi}


`load` remains the effectful step:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_02.aivi{aivi}


See [External Sources](../external_sources.md#128-sourceerror) for the shared error model.

## The stable v0.1 schema strategy

Current v0.1 public docs and tooling center on one stable schema-first field:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_03.aivi{aivi}


`source.schema.derive` means “use the declaration's result type as the source contract.” That is why top-level schema-first bindings should carry an explicit `Source K A` type signature.

### Derived schema example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_06.aivi{aivi}


The type annotation on `usersSource` matters because it gives the compiler and LSP a concrete contract to explain before any later `load usersSource`.

### About external contract artifacts

Checked-in schema files, OpenAPI documents, and database schema values can still be part of a source workflow, but they should stay explicit. Use [Compile-Time Sources](compile_time.md) and [`@static`](../decorators/static.md#static-compile-time-evaluation) when you want contract data fixed at build time, and use the [Database Domain](../../stdlib/system/database.md) for today's table and query APIs.

This page intentionally documents the stable schema-first surface that is available across the current public docs and implementation. Additional connector-specific `source.schema.*` carriers should be treated as connector-specific syntax only after their own guide defines the exact form and verification story.

## Typed connector configuration

Schema-first sources use typed connector records rather than unstructured option bags. The exact fields depend on the connector, but the stable public pattern today looks like this:

| Connector family | Typical record fields | Stable schema field today |
| --- | --- | --- |
| File / JSON / CSV | `path` plus connector-specific file options | `schema: source.schema.derive` |
| Environment | `prefix` and environment-decoding options such as `strict` | `schema: source.schema.derive` |
| REST / HTTP | request fields such as `url`, `timeoutMs`, `strictStatus`, headers/auth, and connector-specific fetch options | `schema: source.schema.derive` |

That makes connector validation local and readable: the record says where data comes from, and the `schema` field says how the result type should drive decoding.

### Environment example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_08.aivi{aivi}


Here the record makes both the source boundary (`prefix: "AIVI_APP"`) and the decoding policy (`strict: True`) visible before any call to `load`.

### REST example

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_09.aivi{aivi}


Here `timeoutMs` and `strictStatus` are connector-specific request details, while `schema: source.schema.derive` still says that the `List User` result type is the decode contract.

## What can be checked before runtime?

Schema-first declarations are most helpful when the source is fully spelled out in one place: a typed `Source ...` signature, a typed connector record, and any helper inputs that are stable at build time.

In the stable v0.1 surface documented here, that lets tooling and the compiler verify things such as:

1. whether `source.schema.derive` has enough type information,
2. whether the connector record matches the constructor you chose,
3. whether a declaration is descriptive enough for hover and diagnostics at the source definition site.

Typical diagnostics include:

- missing explicit type information for `source.schema.derive`,
- malformed or incomplete connector config for the chosen source constructor,
- runtime `DecodeError` values when live data still does not match the declared result type.

Compile-time checking does not remove runtime decoding. Live files, env vars, or API responses can still diverge, so `load` remains responsible for the final decode.

## Explicit remote discovery

Remote discovery must stay explicit.

If you want to fetch an OpenAPI document or another remote contract while compiling, do it through an explicit compile-time source such as `openapi.fromUrl`. See [Compile-Time Sources](compile_time.md) and [`@static` OpenAPI sources](../decorators/static.md#openapi-source). The compiler should not perform surprise network discovery just because a source declaration exists.

## Moving from inline `load` calls to named declarations

The migration is usually mechanical: keep the same record-shaped source, move it into a named `Source ...` binding, and load that binding later.

### File / JSON

Before:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_04.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_05.aivi{aivi}


### Environment

Before:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_06.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_07.aivi{aivi}


### REST / HTTP

Before:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_08.aivi{aivi}


After:

<<< ../../snippets/from_md/syntax/external_sources/schema_first/block_09.aivi{aivi}


For database-backed reads, use the current [Database Domain](../../stdlib/system/database.md) APIs until a connector guide defines a stable schema-first database record form on its own page.

## How this relates to source composition

Schema-first declarations answer “what source are we naming, and what result type should it decode into?”

Source composition answers “once that named source exists, what extra policy should wrap `load`?”

Use schema-first declarations for:

- named, reusable external boundaries,
- explicit connector config,
- a clear decode contract before the first `load`.

Use source composition for:

- transforms,
- semantic validation,
- retry and timeout policy,
- caching,
- provenance and observation.

In practice, the order is: declare the source first, then wrap that source with composition helpers as needed. See [Source Composition](composition.md) for those execution-stage details.
