# External Sources

External data enters AIVI through typed **Sources**. A source represents a persistent connection or a one-off fetch to an external system, with full type safety enforced during decoding.

## 12.1 The Source Type

<<< ../snippets/from_md/02_syntax/12_external_sources/block_01.aivi{aivi}

- `K` — the **kind** of source (File, Http, Db, etc.)
- `A` — the **decoded type** of the content

Sources are effectful. Loading a source performs I/O and returns an `Effect E A` (where `E` captures the possible source errors). All source interactions must occur within an `effect` block.

Typical API shape:

<<< ../snippets/from_md/02_syntax/12_external_sources/block_02.aivi{aivi}

To handle errors as values, use `attempt` (see [Effects](09_effects.md)):

<<< ../snippets/from_md/02_syntax/12_external_sources/block_03.aivi{aivi}


## 12.2 File Sources

Used for local system access. Supports structured (JSON, CSV) and unstructured (Bytes, Text) data.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_04.aivi{aivi}


## 12.3 HTTP Sources

Typed REST/API integration.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_05.aivi{aivi}


## 12.4 Environment Sources (Env)

Typed access to environment configuration. Values are decoded using the expected type and optional defaults.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_06.aivi{aivi}

## 12.5 Database Sources (Db)

Integration with relational and document stores. Uses carrier-specific domains for querying.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_07.aivi{aivi}

See the [Database Domain](../05_stdlib/03_system/23_database.md) for table operations, deltas, and migrations.


## 12.6 Email Sources

Interacting with mail servers (IMAP/SMTP).

<<< ../snippets/from_md/02_syntax/12_external_sources/block_08.aivi{aivi}


## 12.7 LLM Sources

AIVI treats Large Language Models as typed probabilistic sources. This is a core part of the AIVI vision for intelligent data pipelines.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_09.aivi{aivi}


## 12.8 Image Sources

Images are typed by their metadata and pixel data format.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_10.aivi{aivi}


## 12.9 S3 / Cloud Storage Sources

Integration with object storage.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_11.aivi{aivi}

> [!NOTE]
> Browser sources are part of the AIVI long-term vision for end-to-end automation but are considered **Experimental** and may not be fully available in the initial WASM-targeted phase.


## 12.10 Compile-Time Sources (@static)

Some sources are resolved at compile time and embedded into the binary. This ensures zero latency/failure at runtime.

<<< ../snippets/from_md/02_syntax/12_external_sources/block_12.aivi{aivi}
