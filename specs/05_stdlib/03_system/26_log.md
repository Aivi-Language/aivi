# Log Domain

The `Log` domain provides **Structured Logging** for modern observability.

`print()` is fine for debugging, but production software needs data. This domain lets you attach metadata (like `{ userId: 123 }`) to your logs, making them machine-readable and ready for ingestion by tools like Datadog or Splunk.

## Overview

```aivi
use aivi (logger)

logger.info "Server started" [("port", "8080"), ("env", "prod")]
```

## Types

```aivi
type Level = Trace | Debug | Info | Warn | Error
type Context = List (Text, Text)
```

## Record Fields

```aivi
logger.log   : Level -> Text -> Context -> Effect Text Unit
logger.trace : Text -> Context -> Effect Text Unit
logger.debug : Text -> Context -> Effect Text Unit
logger.info  : Text -> Context -> Effect Text Unit
logger.warn  : Text -> Context -> Effect Text Unit
logger.error : Text -> Context -> Effect Text Unit
```

## Goals for v1.0

- Standard levels: `Trace`, `Debug`, `Info`, `Warn`, `Error`.
- Structured context (key-value pairs) rather than just format strings.
- Pluggable backends (console by default, WASI logging).
