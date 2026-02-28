# HTTPS Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.https"} -->
The `Https` domain mirrors `Http`, but enforces secure (TLS) connections. It is intended for production use where transport security is required.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.https</div>

<<< ../../snippets/from_md/stdlib/network/https/https_domain.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect Text (Result Error Response)`</code></pre> | Performs a secure GET request and returns a `Response`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect Text (Result Error Response)`</code></pre> | Performs a secure POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect Text (Result Error Response)`</code></pre> | Performs a secure request with custom method, headers, and body. |

## Types

`aivi.net.https` exports its own type definitions, identical in shape to `aivi.net.http`.

### `Header`

```aivi
Header = { name: Text, value: Text }
```

### `Body`

```aivi
Body = Plain Text | Form (List Header)
```

### `Request`

```aivi
Request = {
  method: Text
  url: Url
  headers: List Header
  body: Option Body
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | HTTP method (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`). |
| `url` | `Url` | Target URL. |
| `headers` | `List Header` | Request headers. |
| `body` | `Option Body` | Optional request body (`Plain Text` or `Form (List Header)`). |

### `Response`

```aivi
Response = {
  status: Int
  headers: List Header
  body: Text
}
```

### `Error`

```aivi
Error = { message: Text }
```
