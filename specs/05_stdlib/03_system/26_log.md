# Log Domain

The `Log` domain provides **Structured Logging** for modern observability.

`print()` is fine for debugging, but production software needs data. This domain lets you attach metadata (like `{ userId: 123 }`) to your logs, making them machine-readable and ready for ingestion by tools like Datadog or Splunk.

## Overview

<<< ../../snippets/from_md/05_stdlib/03_system/26_log/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/03_system/26_log/block_02.aivi{aivi}

## Record Fields

<<< ../../snippets/from_md/05_stdlib/03_system/26_log/block_03.aivi{aivi}

## Goals for v1.0

- Standard levels: `Trace`, `Debug`, `Info`, `Warn`, `Error`.
- Structured context (key-value pairs) rather than just format strings.
- Pluggable backends (console by default, WASI logging).
