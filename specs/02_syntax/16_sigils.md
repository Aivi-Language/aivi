# Sigils

AIVI supports custom literal syntax via **sigils**, inspired by Elixir.

## Overview

Sigils allow domains to define custom parsing logic for string content with flexible delimiters. Common built-in tags include `~r` for regular expressions and `~u` for URIs.

```aivi
let pattern = ~r/\w+@\w+\.\w+/
// -> Regex

let endpoint = ~u(https://api.example.com)
// -> Url
```

## Boundaries (Delimiters)

Sigils support various delimiters to avoid escaping collisions:
*   `~r/.../` (Standard)
*   `~r"..."`
*   `~r(...)`
*   `~r[...]`
*   `~r{...}`

## Built-in Sigils

### `~r` (Regex)

Constructs a `Regex` object.
- **Validation**: The string content is parsed as a regular expression at compile-time.
- **Escaping**: Raw string semantics (backslashes are preserved).

### `~u` (URL)

Constructs a `Url` object.
- **Validation**: Verified to be a valid URL.
- **Type**: Returns a `Url` record `{ protocol, host, path, ... }` (see `std.Url`).

## Custom Sigils

(Future work: allowing user-defined domains to register new sigils.)
