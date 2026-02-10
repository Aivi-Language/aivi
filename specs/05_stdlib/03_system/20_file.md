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

```aivi
open : String -> Effect (Resource Handle)
```

Opens a file for reading. Returns a `Handle` resource that must be managed (e.g., with `resource` block).

### `readAll`

```aivi
readAll : Handle -> Effect (Result String Error)
```

Reads the entire content of an open file handle.

### `close`

```aivi
close : Handle -> Effect Unit
```

Closes the file handle. (Automatically called if using `resource` block).

## Path Operations

### `readText`

```aivi
readText : String -> Effect (Result String Error)
```

Reads the entire content of a file as a string.

### `writeText`

```aivi
writeText : String -> String -> Effect (Result Unit Error)
```

Writes a string to a file, overwriting it if it exists.

### `exists`

```aivi
exists : String -> Effect Bool
```

Checks if a file or directory exists at the given path.

### `stat`

```aivi
stat : String -> Effect (Result FileStats Error)
```

Retrieves metadata about a file or directory. Fails if path does not exist.

### `delete`

```aivi
delete : String -> Effect (Result Unit Error)
```

Deletes a file.
