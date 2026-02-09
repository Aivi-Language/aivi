# File Domain

This domain handles **File System Operations** defined as Side Effects.

## What is this?

Your code lives in memory, but data lives on disk. This domain bridges the gap. It allows you to:
*   **Read/Write**: Load text from a config file or save user data.
*   **Check**: See if a file exists before trying to open it.
*   **Inspect**: Get metadata like "How big is this file?" or "When was it last modified?"

## Why this exists

Direct file access is dangerous (what if the file is locked? missing? corrupted?). By wrapping these operations in `Effect (Result ...)` types, AIVI forces you to handle failure cases (like "File Not Found") explicitely, making your programs crash-proof against disk errors.

## Overview

```aivi
use aivi.std.system.file use { read_text, stat }

// Safe reading
let content = read_text "config.json"

// Metadata inspection
match stat "large_video.mp4" {
    | Ok info => print "File size: ${info.size} bytes"
    | Err _   => print "File not found"
}
```

## Types

```aivi
FileStats = {
  size: Int          // Size in bytes
  created: Int       // Unix timestamp (ms)
  modified: Int      // Unix timestamp (ms)
  is_file: Bool
  is_directory: Bool
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

### `read_text`

```aivi
read_text : String -> Effect (Result String Error)
```

Reads the entire content of a file as a string.

### `write_text`

```aivi
write_text : String -> String -> Effect (Result Unit Error)
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
