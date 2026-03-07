# File Domain

<!-- quick-info: {"kind":"module","name":"aivi.file"} -->
The `File` domain lets an AIVI program read from and write to the filesystem in a controlled way.

Use it for common tasks such as loading configuration, saving generated output, reading large files, checking whether a path exists, or inspecting file metadata.

<!-- /quick-info -->
<div class="import-badge">use aivi.file</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/file/overview.aivi{aivi}

File operations can fail for ordinary reasons: a file may not exist, a directory may be missing, or the program may not have permission to read or write. AIVI models those cases explicitly with `Effect FileError ...` so the failure path is part of the program, not a surprise crash.

## Types

<<< ../../snippets/from_md/stdlib/system/file/types.aivi{aivi}

## Capabilities

- `open`, `readAll`, `readText`, `readJson`, `readCsv`, `readBytes`, `readLines`, `imageMeta`, and `image` require file-read capability.
- `exists`, `stat`, and `listDir` require file-metadata capability.
- `writeText`, `writeBytes`, `appendText`, `delete`, `mkdir`, `mkdirAll`, and `rmdir` require file-write capability.
- `copy` and `move` need both read and write access.

## Choosing between resource and path APIs

- Use the **path-based helpers** such as `readText` and `writeText` for short, direct operations.
- Use the **resource API** when you want tighter control over a handle or when a workflow naturally lives inside a `resource` block.
- Use `readLines` when you want to process a file incrementally instead of loading all text at once.

## Resource operations

### `open`

| Function | What it does |
| --- | --- |
| **open** path<br><code>Text -> Resource FileError Handle</code> | Opens a file for reading and returns a managed `Handle`. |

### `readAll`

| Function | What it does |
| --- | --- |
| **readAll** handle<br><code>Handle -> Effect FileError Text</code> | Reads the complete contents of an open handle as text. |

### `close`

| Function | What it does |
| --- | --- |
| **close** handle<br><code>Handle -> Effect FileError Unit</code> | Closes a file handle manually. Inside `resource`, cleanup is automatic. |

## Common path-based reads

| Function | What it does |
| --- | --- |
| **readText** path<br><code>Text -> Effect FileError Text</code> | Reads an entire file as text. |
| **readJson** path<br><code>Text -> Effect FileError A</code> | Reads JSON from `path` and decodes it into the expected type `A`. |
| **readCsv** path<br><code>Text -> Effect FileError (List A)</code> | Reads CSV rows and decodes them into typed values. |
| **imageMeta** path<br><code>Text -> Effect FileError A</code> | Reads image metadata such as width, height, and format into the expected type. |
| **image** path<br><code>Text -> Effect FileError A</code> | Loads image data, including pixels, into the expected typed shape. |
| **readBytes** path<br><code>Text -> Effect FileError Bytes</code> | Reads an entire file as raw bytes. |
| **readLines** path<br><code>Text -> Effect FileError (Generator Text)</code> | Returns a lazy generator of lines for incremental processing. |

## Common path-based writes and checks

| Function | What it does |
| --- | --- |
| **writeText** path contents<br><code>Text -> Text -> Effect FileError Unit</code> | Writes `contents` to `path`, replacing any existing file. |
| **writeBytes** path bytes<br><code>Text -> Bytes -> Effect FileError Unit</code> | Writes raw bytes to `path`, replacing any existing file. |
| **appendText** path contents<br><code>Text -> Text -> Effect FileError Unit</code> | Adds `contents` to the end of an existing file. |
| **exists** path<br><code>Text -> Effect FileError Bool</code> | Returns `True` when a file or directory exists at `path`. |
| **stat** path<br><code>Text -> Effect FileError FileStats</code> | Returns metadata about a file or directory. |
| **delete** path<br><code>Text -> Effect FileError Unit</code> | Removes a file. |
| **copy** src dest<br><code>Text -> Text -> Effect FileError Unit</code> | Copies a file from `src` to `dest`. |
| **move** src dest<br><code>Text -> Text -> Effect FileError Unit</code> | Moves or renames a file. |

## Directory operations

| Function | What it does |
| --- | --- |
| **listDir** path<br><code>Text -> Effect FileError (List Text)</code> | Lists the names of entries inside a directory. |
| **mkdir** path<br><code>Text -> Effect FileError Unit</code> | Creates one directory and fails if parent directories are missing. |
| **mkdirAll** path<br><code>Text -> Effect FileError Unit</code> | Creates a directory and any missing parents. |
| **rmdir** path<br><code>Text -> Effect FileError Unit</code> | Removes an empty directory. |

## Practical notes

- Relative paths are resolved against the program's current working directory.
- `readLines` is a good fit for large log files, exports, or streamed text processing.
- When you already need a resource scope for other reasons, `open` can make multi-step file workflows easier to structure.
