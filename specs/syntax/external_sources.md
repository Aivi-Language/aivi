# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with decoding intended to be type-driven at the boundary (see the v0.1 status note below).

> [!NOTE]
> v0.1 status:
> - Implemented: `Source K A`, `load`, `file.read`, `http.get`/`post`/`fetch` (and `https.*`), and `env.get` (single-variable reads).
> - Out of scope in runtime v0.1: structured codecs like `file.json`/`file.csv`, streaming sources, and higher-level decoding helpers like `env.decode`.
> - `SourceError K` is upgraded from `Text` to an ADT supporting `DecodeError` accumulation.

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


## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

<<< ../snippets/from_md/syntax/external_sources/file_sources.aivi{aivi}


## 12.3 HTTP Sources

Typed REST/API integration.

<<< ../snippets/from_md/syntax/external_sources/http_sources.aivi{aivi}


## 12.4 Environment Sources (Env)

Typed access to environment configuration. Values are decoded using the expected type and optional defaults.

<<< ../snippets/from_md/syntax/external_sources/environment_sources_env.aivi{aivi}

## 12.5 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

<<< ../snippets/from_md/syntax/external_sources/database_sources_db.aivi{aivi}

See the [Database Domain](../stdlib/system/database.md) for table operations, deltas, and migrations.


## 12.6 Email Sources

Interacting with mail servers (IMAP/SMTP).

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
