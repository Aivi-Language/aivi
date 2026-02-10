# HTTP Domain

The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

```aivi
use aivi.std.network.http
```

## Functions

### `get`

```aivi
get : Url -> Effect (Result Response Error)
```

Performs a GET request to the specified URL.

### `post`

```aivi
post : Url -> Text -> Effect (Result Response Error)
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
    headers: List { name: Text, value: Text },
    body: Text
}
```

### `Request`

```aivi
type Request = {
    method: Text,
    url: Url,
    headers: List { name: Text, value: Text },
    body: Option Text
}
```
