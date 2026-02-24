# Log Module

> **Status: Implemented in runtime v0.1.**

<!-- quick-info: {"kind":"module","name":"aivi.log"} -->
The `Log` module provides **Structured Logging** for modern observability.

`print()` is fine for debugging, but production software needs data. This module lets you attach metadata (like `{ userId: 123 }`) to your logs, making them machine-readable and ready for ingestion by tools like Datadog or Splunk.

<!-- /quick-info -->
<div class="import-badge">use aivi.log</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/log/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/log/types.aivi{aivi}

## Core API (v0.1)

### Logging functions

| Function | Explanation |
| --- | --- |
| **logger.log** level message context<br><pre><code>`Level -> Text -> Context -> Effect Text Unit`</code></pre> | Emits a log entry at the given `level` with `message` and key-value `context`. |
| **logger.trace** message context<br><pre><code>`Text -> Context -> Effect Text Unit`</code></pre> | Shorthand for `logger.log Trace`. |
| **logger.debug** message context<br><pre><code>`Text -> Context -> Effect Text Unit`</code></pre> | Shorthand for `logger.log Debug`. |
| **logger.info** message context<br><pre><code>`Text -> Context -> Effect Text Unit`</code></pre> | Shorthand for `logger.log Info`. |
| **logger.warn** message context<br><pre><code>`Text -> Context -> Effect Text Unit`</code></pre> | Shorthand for `logger.log Warn`. |
| **logger.error** message context<br><pre><code>`Text -> Context -> Effect Text Unit`</code></pre> | Shorthand for `logger.log Error`. |

Notes:
- `Context` is `List (Text, Text)` — a simple key-value pair list attached to each log output.
- `aivi.log` also exports convenience wrappers `log`, `trace`, `debug`, `info`, `warn`, and `error` with the same signatures as `logger.*`.
- The runtime provides a default log handler that writes to stdout. Custom handlers can be injected via effect handlers.

## Type Signatures

<<< ../../snippets/from_md/stdlib/system/log/type_signatures.aivi{aivi}
