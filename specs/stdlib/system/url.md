# URL Domain

<!-- quick-info: {"kind":"module","name":"aivi.url"} -->
The `Url` domain treats web addresses as structured values instead of plain text.

Use `~u(...)` / `~url(...)` for literals and `parse` for runtime text so you can inspect, transform, and render URLs without relying on fragile string concatenation.

<!-- /quick-info -->
<div class="import-badge">use aivi.url<span class="domain-badge">domain</span></div>

## Module

<<< ../../snippets/from_md/stdlib/system/url/module.aivi{aivi}

`Url` is an [opaque type](../../syntax/types/opaque_types.md). Outside `aivi.url`, create values with `parse` or a URL sigil, inspect them with exported accessors, and change query parameters with the exported domain operators. Direct field access and record update are intentionally unavailable.

For most code, the workflow is:

1. get a `Url` from a sigil or `parse`,
2. read pieces with accessors such as `host` or `query`,
3. add or remove query parameters with `+` / `-`,
4. call `toString` only when you need plain `Text` again.

## Types

<<< ../../snippets/from_md/stdlib/system/url/types.aivi{aivi}

A `Url` keeps the major parts separate:

- `protocol` stores the URL scheme, such as `http`, `https`, or `wss`.
- `host` stores the host name or IP literal.
- `port` is `Some ...` when the URL includes an explicit port.
- `path` stores the resource path. For origin-only HTTP(S) URLs such as `https://example.com`, this is `/`.
- `query` stores query parameters as ordered `(key, value)` pairs. Duplicate keys are preserved.
- `hash` stores the optional fragment without the leading `#`.

## URL literal syntax

`~u(...)` and `~url(...)` are compiler-provided structured sigils. Use a sigil when the full URL is known at compile time; use `parse` when it comes from config, input, or another runtime source.

Both forms validate the URL as an absolute URL. A bad sigil is a compile-time error, while `parse` returns `Err ...` at runtime.

See also [Operators: Sigils](../../syntax/operators.md#118-sigils).

## Domain Definition

Inside `aivi.url`, the domain is defined like this:

<<< ../../snippets/from_md/stdlib/system/url/block_01.aivi{aivi}


Outside the module, use the exported operators (`url + ("page", "1")`, `url - "page"`). Do not rely on record update syntax against `Url`; the representation is hidden outside `aivi.url`. The `-` operator removes every query entry whose key matches the given text.

## Why use structured URLs

URLs contain several parts—protocol, host, path, query, and sometimes a fragment.
Building them by hand as text often leads to small mistakes such as double slashes, missing separators, or unescaped values.
Working with a structured `Url` value makes those operations clearer, and it lets APIs such as [`aivi.net.http`](../network/http.md) keep accepting typed URLs instead of raw `Text`.

## Helper functions

| Function | What it does |
| --- | --- |
| **parse** text<br><code>Text -> Result Text Url</code> | Parses text into a structured `Url`. Invalid input returns `Err` with a human-readable message instead of a malformed value. |
| **toString** url<br><code>Url -> Text</code> | Renders a `Url` value back into its string form. |
| **protocol** url<br><code>Url -> Text</code> | Returns the protocol (scheme) such as `"https"` or `"wss"`. |
| **host** url<br><code>Url -> Text</code> | Returns the host name or IP literal. |
| **port** url<br><code>Url -> Option Int</code> | Returns the explicit port when one was present in the URL. |
| **path** url<br><code>Url -> Text</code> | Returns the path component. |
| **query** url<br><code>Url -> List (Text, Text)</code> | Returns query parameters as ordered key/value pairs. |
| **hash** url<br><code>Url -> Option Text</code> | Returns the fragment when one was present. |

`parse` currently returns `Result Text Url`, so handle parse failures anywhere raw text enters your program and convert the message into a domain-specific error if you need one.

## Common patterns

- Use `~u(...)` or `~url(...)` for hard-coded URLs and `parse` for runtime text.
- Keep values as `Url` while your program is transforming them, and use `+` / `-` for query parameters.
- Remember that `url - "page"` removes all `page` entries, not just the first one.
- Use the accessor functions instead of assuming anything about the hidden representation.
- Render with `toString` only at a text boundary such as logging, serialization, or displaying the final address.

## Usage Examples

<<< ../../snippets/from_md/stdlib/system/url/block_02.aivi{aivi}


<<< ../../snippets/from_md/stdlib/system/url/block_03.aivi{aivi}


## Related modules

- [`aivi.net.http`](../network/http.md) and [`aivi.net.https`](../network/http.md#https-domain) for HTTP requests that already accept `Url`
- [`aivi.rest`](../network/rest.md) for higher-level JSON API calls built on top of HTTP
- [`aivi.path`](./path.md) for the same “structured value instead of raw text” approach for filesystem paths
- [Opaque Types](../../syntax/types/opaque_types.md) for the rules behind `Url`'s hidden representation
