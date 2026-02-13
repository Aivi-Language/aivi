# System Domain

The `System` domain connects your program to the operating system.

It allows you to read **Environment Variables** (like secret queries or API keys), handle command-line arguments, or signal success/failure with exit codes. It is the bridge between the managed AIVI runtime and the chaotic host machine.

## Overview

```aivi
use aivi.system (env)

main = effect {
  // Read an environment variable (missing => default)
  portOpt <- env.get "PORT"
  port =
    portOpt ?
      | Some p => p
      | None   => "8080"
  print port
}
```

## Values

```aivi
env : {
  get: Text -> Effect Text (Option Text)
  set: Text -> Text -> Effect Text Unit
  remove: Text -> Effect Text Unit
}

args : Effect Text (List Text)
localeTag : Effect Text (Option Text)
exit : Int -> Effect Text Unit
```

## Goals for v1.0

- Environment variables (read-only or read-write depending on capabilities).
- Command-line arguments.
- Process termination (`exit`).
- Spawning child processes (optional for v1.0, but good to plan).
