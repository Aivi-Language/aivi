# File Domain

The `File` domain bridges the gap between your code and the disk.

Your code lives in ephemeral memory, but data needs to persist. This domain lets you safely read configs, save user data, and inspect directories.
*   **Read/Write**: Load a config or save a savegame.
*   **Check**: "Does this file exist?"
*   **Inspect**: "When was this modified?"

Direct file access is dangerous (locks, missing files, permissions). AIVI wraps these in `Effect` types, forcing you to handle errors explicitly. Your program won't crash just because a file is missing; it will handle it.

## Overview

```aivi
use aivi.file (readText, stat)

// Safe reading
content = readText "config.json"

// Metadata inspection
match stat "large_video.mp4" {
    | Ok info => print "File size: {info.size} bytes"
    | Err _   => print "File not found"
}
```

## Types

```aivi
FileStats = {
  size: Int          // Size in bytes
  created: Int       // Unix timestamp (ms)
  modified: Int      // Unix timestamp (ms)
  isFile: Bool
  isDirectory: Bool
}
```

## Resource Operations

For more control or large files, use the resource-based API.

### `open`


| Function | Explanation |
| --- | --- |
| **open** path<br><pre><code>`String -> Effect (Resource Handle)`</code></pre> | Opens a file for reading and returns a managed `Handle` resource. |

### `readAll`


| Function | Explanation |
| --- | --- |
| **readAll** handle<br><pre><code>`Handle -> Effect (Result String Error)`</code></pre> | Reads the entire contents of an open handle as a string. |

### `close`


| Function | Explanation |
| --- | --- |
| **close** handle<br><pre><code>`Handle -> Effect Unit`</code></pre> | Closes the file handle (automatic with `resource` blocks). |

## Path Operations

### `readText`


| Function | Explanation |
| --- | --- |
| **readText** path<br><pre><code>`String -> Effect (Result String Error)`</code></pre> | Reads the entire contents of `path` as a string. |

### `writeText`


| Function | Explanation |
| --- | --- |
| **writeText** path contents<br><pre><code>`String -> String -> Effect (Result Unit Error)`</code></pre> | Writes `contents` to `path`, overwriting if it exists. |

### `exists`


| Function | Explanation |
| --- | --- |
| **exists** path<br><pre><code>`String -> Effect Bool`</code></pre> | Returns whether a file or directory exists at `path`. |

### `stat`


| Function | Explanation |
| --- | --- |
| **stat** path<br><pre><code>`String -> Effect (Result FileStats Error)`</code></pre> | Retrieves metadata about a file or directory at `path`. |

### `delete`


| Function | Explanation |
| --- | --- |
| **delete** path<br><pre><code>`String -> Effect (Result Unit Error)`</code></pre> | Removes the file at `path`. |
