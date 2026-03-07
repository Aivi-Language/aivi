# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with decoding intended to be type-driven at the boundary (see the v0.1 status note below).

> [!NOTE]
> v0.1 status:
> - Implemented: `Source K A`, `load`, `file.read`/`json`/`csv`, `file.imageMeta`/`image`, `http.get`/`post`/`fetch` (and `https.*`), `rest.*`, `env.get`, and `env.decode`.
> - Streaming sources remain out of scope in runtime v0.1.
> - `SourceError K` is upgraded from `Text` to an ADT supporting `DecodeError` accumulation.
> - Phase 3 adds the schema-first declaration and source-composition models specified in [Schema-First Source Definitions](external_sources/schema_first.md) and [Source Composition](external_sources/composition.md); compiler/runtime work for those models lands separately.

## Source Guides

- [Schema-First Source Definitions](external_sources/schema_first.md)
- [Source Composition](external_sources/composition.md)
- [File Sources](external_sources/file.md)
- [REST / HTTP Sources](external_sources/rest_http.md)
- [Environment Sources](external_sources/environment.md)
- [IMAP Email Sources](external_sources/imap_email.md)
- [Image Sources](external_sources/image.md)
- [Compile-Time Sources](external_sources/compile_time.md)

## 12.1 The Source Type

<<< ../snippets/from_md/syntax/external_sources/the_source_type.aivi{aivi}

- `K`   the **kind** of source (File, Http, Db, etc.)
- `A`   the **decoded type** of the content

### SourceError
A `Source K A` yields an `Effect (SourceError K) A`. The error encapsulates transport boundaries vs structural bounds.

```aivi
SourceError K = 
  | IOError Text
  | DecodeError (List aivi.validation.DecodeError)
```

Sources are effectful. Loading a source performs I/O and returns an `Effect E A` (where `E` captures the possible source errors). All source interactions must occur within a `do Effect { ... }` block.

Typical API shape:

<<< ../snippets/from_md/syntax/external_sources/sourceerror_01.aivi{aivi}

To handle errors as values, use `attempt` (see [Effects](effects.md)):

<<< ../snippets/from_md/syntax/external_sources/sourceerror_02.aivi{aivi}

### Capability mapping (Phase 1 surface)

`Source K A` is pure description data. The capability requirement appears when the source is **loaded**:

- `load (file.*)` / `load (file.image*)` → `file.read`
- `load (rest.*)` / `load (http.*)` / `load (https.*)` → `network.http`
- `load (env.*)` → `process.env.read`
- `load (email.imap ...)` and other mail/network connectors → `network`
- database-backed source reads → `db.query`
- `@static` embedded sources → no runtime capability after compilation

See [Capabilities](capabilities.md) for the standard vocabulary.

## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

<<< ../snippets/from_md/syntax/external_sources/file_sources.aivi{aivi}


## 12.3 HTTP Sources

Typed REST/API integration.
In v0.1 this is available through `http.*`/`https.*` and the `aivi.rest` facade.

<<< ../snippets/from_md/syntax/external_sources/http_sources.aivi{aivi}


## 12.4 Environment Sources (Env)

Typed access to environment configuration. Values are decoded using the expected type and optional defaults.

<<< ../snippets/from_md/syntax/external_sources/environment_sources_env.aivi{aivi}

## 12.5 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

> **v0.1 note:** The schema-first `Source Db` declaration and raw-SQL `db.query` external source shown below are **not yet implemented** in v0.1.  Use the typed `do Query { ... }` DSL in `aivi.database` for in-memory queries (see [Database Domain](../stdlib/system/database.md)).

<<< ../snippets/from_md/syntax/external_sources/database_sources_db.aivi{aivi}

See the [Database Domain](../stdlib/system/database.md) for table operations, deltas, and migrations.


## 12.6 Email Sources

Interacting with mail servers (IMAP/SMTP).
In v0.1, IMAP reads are available through `email.imap` / `aivi.email`.

<<< ../snippets/from_md/syntax/external_sources/email_sources.aivi{aivi}


## 12.7 LLM Sources

AIVI treats Large Language Models as typed probabilistic sources. This is a core part of the AIVI vision for intelligent data pipelines.

<<< ../snippets/from_md/syntax/external_sources/llm_sources.aivi{aivi}


## 12.8 Image Sources

Images are typed by their metadata and pixel data format.

<<< ../snippets/from_md/syntax/external_sources/image_sources.aivi{aivi}


## 12.9 S3 / Cloud Storage Sources

Integration with object storage.

<<< ../snippets/from_md/syntax/external_sources/s3_cloud_storage_sources.aivi{aivi}

> [!NOTE]
> Browser sources are **Experimental** and not guaranteed across all v0.1 runtime targets (including WASM).


## 12.10 Compile-Time Sources (@static)

Some sources are resolved at compile time and embedded into the binary. This ensures zero latency/failure at runtime.

<<< ../snippets/from_md/syntax/external_sources/compile_time_sources_static.aivi{aivi}

## 12.11 Source composition (Phase 3)

Phase 3 treats a schema-bearing `Source K A` as a **declaration plus a canonical execution pipeline**:

1. optional cache lookup,
2. connector acquisition wrapped by timeout / retry / backoff policy,
3. schema-driven decode,
4. pure transform stages,
5. accumulated validation stages,
6. cache write + provenance / observability emission.

`load` remains the only effectful step. The composition layers are pure source-description data, just like the underlying connector declaration.

See [Source Composition](external_sources/composition.md) for the public stage model, policy semantics, provenance contract, and handler-based testing story.
