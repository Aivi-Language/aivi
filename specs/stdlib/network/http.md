# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text (Result Error Response)</code> | Performs a GET request and returns a `Response`. |
| **post** url body<br><code>Url -> Text -> Effect Text (Result Error Response)</code> | Performs a POST request with a text body. |
| **fetch** request<br><code>Request -> Effect Text (Result Error Response)</code> | Performs a request with custom method, headers, and body. |

## Types

### `Header`

A single HTTP header as a name/value pair.

```aivi
Header = { name: Text, value: Text }
```

### `Body`

The request body. Use `Plain` for raw text, `Form` for form-encoded data, or `Json` for a JSON-serialised value.

```aivi
Body = Plain Text | Form (List Header) | Json JsonValue
```

When the expected type is `Body`, a plain record literal is automatically coerced to `Json (toJson record)`, so you can write:

```aivi
body: Some { grant_type: "authorization_code", code: code }
```

The `Json` variant automatically sets `Content-Type: application/json` when no `Content-Type` header is already present in the request.

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/request.aivi{aivi}

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | HTTP method (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`). |
| `url` | `Url` | Target URL. |
| `headers` | `List Header` | Request headers. |
| `body` | `Option Body` | Optional request body. `Plain Text`, `Form (List Header)`, or `Json JsonValue` (record coerces automatically). |

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/response.aivi{aivi}

### `Error`

Returned in the `Err` branch when a request fails.

```aivi
Error = { message: Text }
```
