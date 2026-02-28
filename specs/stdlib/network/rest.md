# REST Module

<!-- quick-info: {"kind":"module","name":"aivi.rest"} -->
The `rest` module is a higher-level HTTP client tailored for consuming JSON REST APIs. Compared to `aivi.net.http`, it adds built-in bearer-token auth, per-request timeouts, automatic retry, and strict status checking.

<!-- /quick-info -->
<div class="import-badge">use aivi.rest</div>

## Types

### `Header`

A single HTTP header as a name/value pair.

```aivi
Header = { name: Text, value: Text }
```

### `Request`

A fully-configurable REST request. All fields beyond `method`, `url`, and `headers` are optional.

```aivi
Request = {
  method: Text
  url: Url
  headers: List Header
  body: Option Text
  timeoutMs: Option Int
  retryCount: Option Int
  bearerToken: Option Text
  strictStatus: Option Bool
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | HTTP method (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`). |
| `url` | `Url` | Target URL. |
| `headers` | `List Header` | Additional request headers. |
| `body` | `Option Text` | Optional request body. |
| `timeoutMs` | `Option Int` | Request timeout in milliseconds. `None` uses the runtime default. |
| `retryCount` | `Option Int` | Number of automatic retries on transient failure. `None` disables retries. |
| `bearerToken` | `Option Text` | If present, adds an `Authorization: Bearer <token>` header. |
| `strictStatus` | `Option Bool` | When `Some True`, treats non-2xx responses as errors. |

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect Text A`</code></pre> | Performs a GET request and decodes the response body into the expected type `A`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect Text A`</code></pre> | Performs a POST request with a text body and decodes the response into `A`. |
| **fetch** request<br><pre><code>`Request -> Effect Text A`</code></pre> | Performs a fully-configured REST request and decodes the response into `A`. |
