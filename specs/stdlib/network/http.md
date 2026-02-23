# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect HttpError Response`</code></pre> | Performs a GET request and returns a `Response`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect HttpError Response`</code></pre> | Performs a POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect HttpError Response`</code></pre> | Performs a request with custom method, headers, and body. |

## Request Options

`fetch` supports richer request options through `Request` fields:

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | HTTP method (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`). |
| `url` | `Url` | Target URL. |
| `headers` | `List Header` | Request headers. |
| `body` | `Option Text` | Optional request body. |
| `timeoutMs` | `Option Int` | Global request timeout in milliseconds. |
| `retryCount` | `Option Int` | Number of retry attempts for transient failures. |
| `bearerToken` | `Option Text` | Optional Bearer auth token; mapped to `Authorization` header. |
| `strictStatus` | `Option Bool` | When `True`, non-2xx statuses are treated as request errors. |

## Types

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/response.aivi{aivi}

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/request.aivi{aivi}

## Header Utilities

| Function | Explanation |
| --- | --- |
| **header** name value<br><pre><code>`Text -> Text -> Header`</code></pre> | Creates one header record. |
| **setHeader** name value request<br><pre><code>`Text -> Text -> Request -> Request`</code></pre> | Adds or replaces one header on a request. |
| **contentType** value request<br><pre><code>`Text -> Request -> Request`</code></pre> | Sets the `Content-Type` header. |
