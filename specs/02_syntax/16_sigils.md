# Sigils

AIVI supports custom literal syntax via **sigils**, inspired by Elixir.

## Overview

Sigils allow domains to define custom parsing logic for string content with flexible delimiters. Common built-in tags include `~r` for regular expressions and `~u` for URIs.

```aivi
let pattern = ~r/\w+@\w+\.\w+/
// -> Regex

let pattern_ci = ~r/\w+@\w+\.\w+/i
// -> Regex (case-insensitive)

let endpoint = ~u(https://api.example.com)
// -> Url

let birthday = ~d(1990-12-31)
// -> Date

let timestamp = ~dt(2025-02-08T12:34:56Z)
// -> DateTime
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
- **Flags**: Optional trailing ASCII letters after the closing delimiter (e.g. `~r/.../gmi`).

### `~u` (URL)

Constructs a `Url` object.
- **Validation**: Verified to be a valid URL.
- **Type**: Returns a `Url` record `{ protocol, host, path, ... }` (see `std.Url`).

### `~d` (Date)

Constructs a `Date` value (for the `Calendar` domain).
- **Format**: `YYYY-MM-DD`

### `~dt` / `~t` (DateTime)

Constructs a `DateTime` value (for the `Calendar` domain).
- **Format**: `YYYY-MM-DDTHH:MM:SSZ` (UTC, `Z` suffix)

## Custom Sigils

(Future work: allowing user-defined domains to register new sigils.)
