# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain gives your program the basic building blocks for making HTTP requests. Use it when you want direct control over request methods, headers, and bodies while still working with a small, predictable API.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## What this module is for

`aivi.net.http` is the low-level HTTP client for AIVI programs. It is a good fit when you want to:

- fetch data from an API with `GET`
- send form or JSON data with `POST`
- build a fully custom request with headers, methods, and optional bodies

If you want a more opinionated client for JSON APIs, automatic bearer-token support, and stricter response handling, look at `aivi.rest`.

## Capabilities

`get`, `post`, and `fetch` require the `network.http` capability, or the broader `network` shorthand. The `rest` source facade uses the same capability family.

## Quick start

This example fetches a page and checks whether the request succeeded:

```aivi
use aivi.net.http

loadHomePage = url => do Effect {
  result <- get url
  result match
    | Ok response =>
        pure response.body
    | Err err =>
        fail "Could not load page: {err.message}"
}
```

For requests that need custom headers or a request body, use `fetch`:

```aivi
use aivi.net.http

submitFeedback = url => do Effect {
  response <- fetch {
    method: "POST"
    url: url
    headers: [
      { name: "Accept", value: "application/json" }
    ]
    body: Some (Plain "Great work!")
  }

  response match
    | Ok ok  => pure ok.status
    | Err err => fail "Request failed: {err.message}"
}
```

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text (Result Error Response)</code> | Sends a `GET` request to `url`. Use this for read-only HTTP calls such as loading a page or fetching JSON as text. |
| **post** url body<br><code>Url -> Text -> Effect Text (Result Error Response)</code> | Sends a `POST` request with a text body. This is a convenient helper when you do not need custom headers or methods. |
| **fetch** request<br><code>Request -> Effect Text (Result Error Response)</code> | Sends a request built from a full `Request` record. Use this when you need custom methods, headers, or body handling. |

## Types

### `Header`

A single HTTP header as a name/value pair.

```aivi
Header = { name: Text, value: Text }
```

Typical examples:

```aivi
{ name: "Accept", value: "application/json" }
{ name: "Authorization", value: "Bearer <token>" }
```

### `Body`

`Body` describes what you send to the server.

```aivi
Body = Plain Text | Form (List Header) | Json JsonValue
```

- `Plain Text` sends raw text.
- `Form (List Header)` sends form-style name/value fields.
- `Json JsonValue` sends JSON data.

When the expected type is `Body`, a plain record literal is automatically coerced to `Json (toJson record)`, so you can write:

```aivi
// This record is automatically turned into JSON.
body: Some { grant_type: "authorization_code", code: code }
```

The `Json` variant automatically adds `Content-Type: application/json` when the request does not already define a `Content-Type` header.

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/request.aivi{aivi}

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | The HTTP method, such as `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, or `HEAD`. |
| `url` | `Url` | The destination URL. |
| `headers` | `List Header` | Extra request headers to send with the request. |
| `body` | `Option Body` | An optional request body. Use `None` for requests without a body. |

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/response.aivi{aivi}

`Response` contains three things:

- `status`: the numeric HTTP status code, such as `200` or `404`
- `headers`: the response headers returned by the server
- `body`: the response body as text

This makes it easy to inspect the status code first and then decide how to handle the body.

### `Error`

Returned in the `Err` branch when a request cannot be completed.

```aivi
Error = { message: Text }
```

In practice, this is the value you inspect when:

- a connection cannot be opened
- the remote server is unavailable
- TLS or transport setup fails before a normal response is returned

---

## HTTPS Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.https"} -->
`aivi.net.https` mirrors `aivi.net.http` but always uses TLS. Choose it when you need encrypted connections, which is the normal choice for production APIs and any request carrying private data.
<!-- /quick-info -->
<div class="import-badge">use aivi.net.https</div>

`aivi.net.https` exports the same functions (`get`, `post`, `fetch`) and the same supporting types (`Header`, `Body`, `Request`, `Response`, `Error`) as `aivi.net.http`. The main difference is that the connection is secured with TLS.

### Common use

If an API gives you an `https://...` URL, import `aivi.net.https` and use it the same way you would use `aivi.net.http`:

```aivi
use aivi.net.https

loadProfile = url => get url
```

### Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text (Result Error Response)</code> | Sends a secure `GET` request over TLS. |
| **post** url body<br><code>Url -> Text -> Effect Text (Result Error Response)</code> | Sends a secure `POST` request with a text body. |
| **fetch** request<br><code>Request -> Effect Text (Result Error Response)</code> | Sends a fully custom secure request over TLS. |

### Types

The data types have the same shapes as the ones in `aivi.net.http`:

```aivi
Header = { name: Text, value: Text }
Body   = Plain Text | Form (List Header) | Json JsonValue
Error  = { message: Text }
```
