# HTTP Domain

The `Http` domain provides functions for making HTTP requests.

```aivi
use std.Http
```

## Functions

### `get`

```aivi
get : Url -> Effect (Result Response Error)
```

Performs a GET request to the specified URL.

### `post`

```aivi
post : Url -> String -> Effect (Result Response Error)
```

Performs a POST request with the given body.

### `fetch`

```aivi
fetch : Request -> Effect (Result Response Error)
```

Performs a custom HTTP request.

## Types

### `Response`

```aivi
type Response = {
    status: Int,
    headers: Map String String,
    body: String
}
```

### `Request`

```aivi
type Request = {
    method: String,
    url: Url,
    headers: Map String String,
    body: Option String
}
```
