# HTTPS Domain

The `Https` domain mirrors `Http`, but enforces secure (TLS) connections. It is intended for production use where transport security is required.

```aivi
use aivi.std.network.https
```

## Functions

### `get`

```aivi
get : Url -> Effect (Result Response Error)
```

Performs a secure GET request to the specified URL.

### `post`

```aivi
post : Url -> Text -> Effect (Result Response Error)
```

Performs a secure POST request with the given body.

### `fetch`

```aivi
fetch : Request -> Effect (Result Response Error)
```

Performs a custom HTTPS request.

## Types

Uses the same `Request` and `Response` types as `aivi.std.network.http`.
