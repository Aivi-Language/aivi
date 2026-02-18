# File Domain

<!-- quick-info: {"kind":"module","name":"aivi.file"} -->
The `File` domain bridges the gap between your code and the disk.

Your code lives in ephemeral memory, but data needs to persist. This domain lets you safely read configs, save user data, and inspect directories.
*   **Read/Write**: Load a config or save a savegame.
*   **Check**: "Does this file exist?"
*   **Inspect**: "When was this modified?"

Direct file access is dangerous (locks, missing files, permissions). AIVI wraps these in `Effect` types, forcing you to handle errors explicitly. Your program won't crash just because a file is missing; it will handle it.

<!-- /quick-info -->
## Overview

<<< ../../snippets/from_md/05_stdlib/03_system/20_file/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/03_system/20_file/block_02.aivi{aivi}

## Resource Operations

For more control or large files, use the resource-based API.

### `open`


| Function | Explanation |
| --- | --- |
| **open** path<br><pre><code>`Text -> Resource FileError Handle`</code></pre> | Opens a file for reading and returns a managed `Handle` resource. |

### `readAll`


| Function | Explanation |
| --- | --- |
| **readAll** handle<br><pre><code>`Handle -> Effect FileError Text`</code></pre> | Reads the entire contents of an open handle as text. |

### `close`


| Function | Explanation |
| --- | --- |
| **close** handle<br><pre><code>`Handle -> Effect FileError Unit`</code></pre> | Closes the file handle (automatic with `resource` blocks). |

## Path Operations

### `readText`


| Function | Explanation |
| --- | --- |
| **readText** path<br><pre><code>`Text -> Effect FileError Text`</code></pre> | Reads the entire contents of `path` as text. |

### `writeText`


| Function | Explanation |
| --- | --- |
| **writeText** path contents<br><pre><code>`Text -> Text -> Effect FileError Unit`</code></pre> | Writes `contents` to `path`, overwriting if it exists. |

### `exists`


| Function | Explanation |
| --- | --- |
| **exists** path<br><pre><code>`Text -> Effect FileError Bool`</code></pre> | Returns whether a file or directory exists at `path`. |

### `stat`


| Function | Explanation |
| --- | --- |
| **stat** path<br><pre><code>`Text -> Effect FileError FileStats`</code></pre> | Retrieves metadata about a file or directory at `path`. |

### `delete`


| Function | Explanation |
| --- | --- |
| **delete** path<br><pre><code>`Text -> Effect FileError Unit`</code></pre> | Removes the file at `path`. |
