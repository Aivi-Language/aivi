# Log Module

<!-- quick-info: {"kind":"module","name":"aivi.log"} -->
The `Log` module is for structured application logging.

Instead of printing plain text only, you can attach named fields such as `userId`, `requestId`, or `orderTotal`, which makes logs easier to read in development and much easier to search in production.

<!-- /quick-info -->
<div class="import-badge">use aivi.log</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/log/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/log/types.aivi{aivi}

## Why structured logging helps

Structured logs are useful when the same program runs in many places or handles many requests at once.
A message like `"payment failed"` is more useful when it also carries context such as the account, region, and error code.

In this module, `Context` is `List (Text, Text)`: a simple list of key-value pairs attached to each entry.

## Logging functions

| Function | What it does |
| --- | --- |
| **logger.log** level message context<br><code>Level -> Text -> Context -> Effect Text Unit</code> | Emits a log entry at the chosen `level` with a message and structured context fields. |
| **logger.trace** message context<br><code>Text -> Context -> Effect Text Unit</code> | Shorthand for `logger.log Trace`. |
| **logger.debug** message context<br><code>Text -> Context -> Effect Text Unit</code> | Shorthand for `logger.log Debug`. |
| **logger.info** message context<br><code>Text -> Context -> Effect Text Unit</code> | Shorthand for `logger.log Info`. |
| **logger.warn** message context<br><code>Text -> Context -> Effect Text Unit</code> | Shorthand for `logger.log Warn`. |
| **logger.error** message context<br><code>Text -> Context -> Effect Text Unit</code> | Shorthand for `logger.log Error`. |

`aivi.log` also exports the convenience wrappers `log`, `trace`, `debug`, `info`, `warn`, and `error` with the same signatures as the `logger.*` functions.

## Practical guidance

- Put the human-readable summary in `message`.
- Put searchable details in `context`.
- Prefer stable keys such as `requestId`, `userId`, and `component` so downstream tools can group similar events.
- Use `info` for normal milestones, `warn` for unusual situations, and `error` when work failed or user-visible behavior is affected.

The runtime provides a default handler that writes one JSON log entry per line. `trace`, `debug`, and `info` go to standard output, while `warn` and `error` go to standard error. Custom handlers can be supplied through effect handlers when you need to forward logs somewhere else.

## Type Signatures

<<< ../../snippets/from_md/stdlib/system/log/type_signatures.aivi{aivi}
