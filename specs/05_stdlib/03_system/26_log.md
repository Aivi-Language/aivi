# Log Domain

The `Log` domain provides structured logging facilities suitable for modern observability.

## Why this exists

`print()` is fine for debugging, but production software needs structured logs (levels, timestamps, metadata fields) that can be ingested by observability tools. This domain standardizes logging across libraries and applications.

## Overview

```aivi
import aivi.std.system.log use { info, error }

info("Server started", { port: 8080, env: "prod" })
```

## Goals for v1.0

- Standard levels: `Trace`, `Debug`, `Info`, `Warn`, `Error`.
- Structured context (key-value pairs) rather than just format strings.
- Pluggable backends (console by default, WASI logging).
