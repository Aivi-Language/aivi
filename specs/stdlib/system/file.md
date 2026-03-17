# File Domain

<!-- quick-info: {"kind":"module","name":"aivi.file"} -->
The `aivi.file` module covers the current v0.1 filesystem surface: reading text and structured files, writing whole text files, deleting files, and inspecting basic metadata.

Use it for tasks such as loading configuration, reading JSON or CSV into typed values, saving generated output, checking whether a path exists, or inspecting file metadata. For safer path construction, pair it with [`aivi.path`](./path).

<!-- /quick-info -->
<div class="import-badge">use aivi.file</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/file/overview.aivi{aivi}

In the overview example, `readText` and `stat` each return an outer `Effect Text ...` and an inner `Result Text ...`. That split is intentional: filesystem interaction is effectful, and some helpers also let you keep ordinary “missing file” handling local with `or` or `match`.

## How errors show up

`aivi.file` currently uses three result shapes:

- `readAll`, `readText`, `writeText`, `stat`, and `delete` return `Effect Text (Result Text A)`.
- `readJson`, `readCsv`, `imageMeta`, and `image` return `Effect Text A`. If decoding fails, the effect fails directly.
- `exists` returns `Effect Text Bool`.

If you are choosing an API for a beginner-friendly workflow, a good rule of thumb is:

- use `readText` / `writeText` / `stat` when you want to keep success-or-error handling in ordinary `Result` code,
- use `readJson` / `readCsv` / `imageMeta` / `image` when the whole operation should fail if decoding fails,
- use `exists` for a simple yes/no check.

## Types

<<< ../../snippets/from_md/stdlib/system/file/types.aivi{aivi}

`FileHandle` is an opaque runtime handle. You obtain it from `open`, pass it to `readAll` or `close`, and treat it as a resource rather than a value you inspect directly.

## Choosing between resource and path APIs

- Use the **path-based helpers** such as `readText`, `readJson`, and `writeText` for one-shot operations.
- Use the **resource API** when a workflow already lives inside a `resource` block or when you want to keep an open file handle around briefly for staged work.
- In normal `do Effect` code, bind `handle <- open path` directly. If a handle escapes an inner scope, the resource cleanup for that inner scope has already run.
- `open` currently opens files for reading; it is not a general read/write handle API.

## Resource operations

### `open`

| Function | What it does |
| --- | --- |
| **open** path<br><code>Text -> Resource Text FileHandle</code> | Opens a file for reading and returns a managed `FileHandle`. |

### `readAll`

| Function | What it does |
| --- | --- |
| **readAll** handle<br><code>FileHandle -> Effect Text (Result Text Text)</code> | Reads the complete contents of an open handle as text. |

### `close`

| Function | What it does |
| --- | --- |
| **close** handle<br><code>FileHandle -> Effect Text Unit</code> | Closes a file handle manually. Inside the `resource` returned by `open`, cleanup is automatic. |

## Common path-based reads

| Function | What it does |
| --- | --- |
| **readText** path<br><code>Text -> Effect Text (Result Text Text)</code> | Reads an entire file as text. |
| **readJson** path<br><code>Text -> Effect Text A</code> | Reads JSON from `path` and decodes it into the expected type `A`. |
| **readCsv** path<br><code>Text -> Effect Text (List A)</code> | Reads CSV rows and decodes them into typed values. |
| **imageMeta** path<br><code>Text -> Effect Text A</code> | Reads image metadata such as width, height, and format into the expected typed shape. |
| **image** path<br><code>Text -> Effect Text A</code> | Loads image data, including pixels, into the expected typed shape. |

### Example: typed JSON load

<<< ../../snippets/from_md/stdlib/system/file/block_01.aivi{aivi}


## Path-based writes and metadata

| Function | What it does |
| --- | --- |
| **writeText** path contents<br><code>Text -> Text -> Effect Text (Result Text Unit)</code> | Writes `contents` to `path`, replacing any existing file. |
| **exists** path<br><code>Text -> Effect Text Bool</code> | Returns `True` when a file or directory exists at `path`. |
| **stat** path<br><code>Text -> Effect Text (Result Text FileStats)</code> | Returns metadata about a file or directory. |
| **delete** path<br><code>Text -> Effect Text (Result Text Unit)</code> | Removes a file. |

## Current v0.1 scope

This module currently focuses on whole-file text reads and writes, structured-file decoding, and basic file metadata.

Byte-oriented helpers, directory traversal helpers, and copy/move helpers are not part of the current public `aivi.file` surface yet, so this page does not document them as available APIs.

## Practical notes

- Relative paths are resolved against the program's current working directory.
- `FileStats.created` and `FileStats.modified` are Unix timestamps in milliseconds. On filesystems without a native creation timestamp, `created` falls back to `modified`.
- `readJson`, `readCsv`, `imageMeta`, and `image` are the right fit when you want typed data instead of raw text.
- When you already need a resource scope for other reasons, `open` and `readAll` can make a short multi-step file workflow easier to structure.
