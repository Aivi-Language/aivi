# System Domain

The `System` domain provides access to the environment and process control, abstracting over the host runtime.

## Why this exists

Programs need to interact with their environment: reading configuration from environment variables, handling command-line arguments, or signaling status via exit codes. This domain bridges the gap between the managed AIVI runtime and the OS.

## Overview

```aivi
import aivi.std.system use { Env }

// Read an environment variable
let port = Env.get("PORT") |> Option.default("8080")
```

## Goals for v1.0

- Environment variables (read-only or read-write depending on capabilities).
- Command-line arguments.
- Process termination (`exit`).
- Spawning child processes (optional for v1.0, but good to plan).
