# File Domain

<!-- quick-info: {"kind":"module","name":"aivi.file"} -->
The `File` domain bridges the gap between your code and the disk.

Your code lives in ephemeral memory, but data needs to persist. This domain lets you safely read configs, save user data, and inspect directories.
*   **Read/Write**: Load a config or save a savegame.
*   **Check**: "Does this file exist?"
*   **Inspect**: "When was this modified?"

Direct file access is dangerous (locks, missing files, permissions). AIVI wraps these in `Effect` types, forcing you to handle errors explicitly. Your program won't crash just because a file is missing; it will handle it.

<!-- /quick-info -->
<div class="import-badge">use aivi.file</div>

## Overview

<<< ../../snippets/from_md/stdlib/system/file/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/system/file/types.aivi{aivi}

## Resource Operations

For more control or large files, use the resource-based API.

### `open`


| Function | Explanation |
| --- | --- |
| **open** path<br><code>Text -> Resource FileError Handle</code> | Opens a file for reading and returns a managed `Handle` resource. |

### `readAll`


| Function | Explanation |
| --- | --- |
| **readAll** handle<br><code>Handle -> Effect FileError Text</code> | Reads the entire contents of an open handle as text. |

### `close`


| Function | Explanation |
| --- | --- |
| **close** handle<br><code>Handle -> Effect FileError Unit</code> | Closes the file handle (automatic with `resource` blocks). |

## Path Operations

### `readText`


| Function | Explanation |
| --- | --- |
| **readText** path<br><code>Text -> Effect FileError Text</code> | Reads the entire contents of `path` as text. |
| **readJson** path<br><code>Text -> Effect FileError A</code> | Reads and decodes JSON from `path` into the expected type `A`. |
| **readCsv** path<br><code>Text -> Effect FileError (List A)</code> | Reads and decodes CSV rows from `path` into typed row values. |
| **imageMeta** path<br><code>Text -> Effect FileError A</code> | Reads image metadata (e.g. width/height/format) into the expected type. |
| **image** path<br><code>Text -> Effect FileError A</code> | Loads image data (including pixels) into the expected typed shape. |

### `writeText`


| Function | Explanation |
| --- | --- |
| **writeText** path contents<br><code>Text -> Text -> Effect FileError Unit</code> | Writes `contents` to `path`, overwriting if it exists. |

### `exists`


| Function | Explanation |
| --- | --- |
| **exists** path<br><code>Text -> Effect FileError Bool</code> | Returns whether a file or directory exists at `path`. |

### `stat`


| Function | Explanation |
| --- | --- |
| **stat** path<br><code>Text -> Effect FileError FileStats</code> | Retrieves metadata about a file or directory at `path`. |

### `delete`


| Function | Explanation |
| --- | --- |
| **delete** path<br><code>Text -> Effect FileError Unit</code> | Removes the file at `path`. |

## Directory Operations

| Function | Explanation |
| --- | --- |
| **listDir** path<br><code>Text -> Effect FileError (List Text)</code> | Returns the names of entries in the directory at `path`. |
| **mkdir** path<br><code>Text -> Effect FileError Unit</code> | Creates a directory at `path`. Fails when parent directories are missing. |
| **mkdirAll** path<br><code>Text -> Effect FileError Unit</code> | Creates a directory and all missing parents. |
| **rmdir** path<br><code>Text -> Effect FileError Unit</code> | Removes an empty directory. |

## Additional Path Operations

| Function | Explanation |
| --- | --- |
| **readBytes** path<br><code>Text -> Effect FileError Bytes</code> | Reads the entire contents of `path` as raw `Bytes`. |
| **writeBytes** path bytes<br><code>Text -> Bytes -> Effect FileError Unit</code> | Writes raw `bytes` to `path`, overwriting if it exists. |
| **appendText** path contents<br><code>Text -> Text -> Effect FileError Unit</code> | Appends `contents` to the end of the file at `path`. |
| **copy** src dest<br><code>Text -> Text -> Effect FileError Unit</code> | Copies the file at `src` to `dest`. |
| **move** src dest<br><code>Text -> Text -> Effect FileError Unit</code> | Moves (or renames) the file at `src` to `dest`. |
| **readLines** path<br><code>Text -> Effect FileError (Generator Text)</code> | Returns a lazy generator of lines in the file. |

Notes:
- All path operations resolve relative paths against the current working directory.
- `readLines` returns a [Generator](/stdlib/core/generator) that reads lines lazily — useful for large files.
