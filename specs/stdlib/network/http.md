# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect Text (Result Error Response)`</code></pre> | Performs a GET request and returns a `Response`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect Text (Result Error Response)`</code></pre> | Performs a POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect Text (Result Error Response)`</code></pre> | Performs a request with custom method, headers, and body. |

## Types

### `Header`

A single HTTP header as a name/value pair.

```aivi
Header = { name: Text, value: Text }
```

### `Body`

The request body. Use `Plain` for raw text and `Form` for form-encoded data.

```aivi
Body = Plain Text | Form (List Header)
```

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/request.aivi{aivi}

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | HTTP method (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`). |
| `url` | `Url` | Target URL. |
| `headers` | `List Header` | Request headers. |
| `body` | `Option Body` | Optional request body (`Plain Text` or `Form (List Header)`). |

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/response.aivi{aivi}

### `Error`

Returned in the `Err` branch when a request fails.

```aivi
Error = { message: Text }
```
