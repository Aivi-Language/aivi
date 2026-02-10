# HTTP Domain

The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

```aivi
use aivi.net.http
```

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect (Result Response Error)`</code></pre> | Performs a GET request and returns a `Response` or `Error`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect (Result Response Error)`</code></pre> | Performs a POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect (Result Response Error)`</code></pre> | Performs a request with custom method, headers, and body. |

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
