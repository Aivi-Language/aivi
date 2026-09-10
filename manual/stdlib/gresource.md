# aivi.gresource

Application data types for gresource. This module supplies vocabulary only: it does not load data, watch sources, or implement the task aliases it defines.

## Import

```aivi
use aivi.gresource (
    ResourceError
    ResourcePath
    ResourceTask
    ResourceTextTask
    ResourceBytesTask
    ResourceListTask
)
```

## Overview

| Item | Type | Description |
|------|------|-------------|
| `ResourceError` | type | Things that can go wrong when resolving or decoding a resource |
| `ResourcePath` | `Text` alias | Checked resource path value |
| `ResourceTask A` | `Task ResourceError A` | Generic resource task alias |
| `ResourceTextTask` | `Task ResourceError Text` | Resource task that returns text |
| `ResourceBytesTask` | `Task ResourceError Bytes` | Resource task that returns bytes |
| `ResourceListTask` | `Task ResourceError (List Text)` | Resource task that returns a list of text values |

## Types

### ResourceError

```aivi
type ResourceError =
  | ResourceNotFound Text
  | ResourceDecodeFailed Text
  | ResourceUnavailable
```

These variants describe the common failure cases when loading a bundled resource.

- `ResourceNotFound` — the path does not exist in the resource bundle
- `ResourceDecodeFailed` — the bytes were found, but could not be decoded as requested
- `ResourceUnavailable` — resource access is not available in the current runtime

### ResourcePath

`ResourcePath` is a `Text` alias for application resource paths. It does not prove that a resource exists. This module provides no loader.

### ResourceTask

```aivi
use aivi.gresource (ResourceError)

type ResourceTask A = (Task ResourceError A)
```

Generic alias for resource-related tasks.

### ResourceTextTask

```aivi
use aivi.gresource (ResourceError)

type ResourceTextTask = (Task ResourceError Text)
```

Alias for resource operations that return decoded text, such as CSS or UI markup.

### ResourceBytesTask

```aivi
use aivi.gresource (ResourceError)

type ResourceBytesTask = (Task ResourceError Bytes)
```

Alias for resource operations that return raw bytes, such as images or other binary data.

### ResourceListTask

```aivi
use aivi.gresource (ResourceError)

type ResourceListTask = (Task ResourceError (List Text))
```

Alias for resource-related tasks that return a list of text values.
