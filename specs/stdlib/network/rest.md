# REST Module

<!-- quick-info: {"kind":"module","name":"aivi.rest"} -->
The `rest` module is a higher-level HTTP client for JSON-based APIs. It is designed for the common case where you want to call a REST endpoint, decode the response into a value, and let the module handle details like bearer tokens, timeouts, retries, and status checks.

<!-- /quick-info -->
<div class="import-badge">use aivi.rest</div>

## What this module is for

Use `aivi.rest` when you are talking to a conventional REST API and want a friendlier interface than raw HTTP requests. Compared to `aivi.net.http`, this module adds:

- automatic response decoding into the type you expect
- optional bearer-token authentication
- optional request timeouts
- optional retry counts for transient failures
- optional strict handling of non-2xx status codes

If you need full control over raw HTTP responses, use `aivi.net.http` instead.

## Typical example

This is the style of code `aivi.rest` is meant for:

```aivi
use aivi.rest

loadUser : Url -> Effect Text User
loadUser = url => get url
```

The expected result type (`User` here) tells the module what to decode from the response body.

For more control, build a `Request` value:

```aivi
use aivi.rest

savePost : Url -> Text -> Effect Text SavedPost
savePost = url => token =>
  fetch {
    method: "POST"
    url: url
    headers: [{ name: "Accept", value: "application/json" }]
    body: Some (Plain "{\"title\":\"Hello\"}")
    timeoutMs: Some 5000      // Fail reasonably quickly if the API stops responding.
    retryCount: Some 2        // Retry a couple of transient failures automatically.
    bearerToken: Some token   // Adds Authorization: Bearer <token>.
    strictStatus: Some True   // Treat non-2xx responses as errors.
  }
```

## Types

### `Header`

A single HTTP header as a name/value pair.

```aivi
Header = { name: Text, value: Text }
```

### `Request`

`Request` gathers the options you can set for a REST call.

```aivi
Body = Plain Text | Form (List Header)

Request = {
  method: Text
  url: Url
  headers: List Header
  body: Option Body
  timeoutMs: Option Int
  retryCount: Option Int
  bearerToken: Option Text
  strictStatus: Option Bool
}
```

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | The HTTP method, such as `GET`, `POST`, `PUT`, `PATCH`, or `DELETE`. |
| `url` | `Url` | The endpoint to call. |
| `headers` | `List Header` | Extra HTTP headers to send with the request. |
| `body` | `Option Body` | Optional request body. Use `Plain` for raw text or `Form` for form-style fields. |
| `timeoutMs` | `Option Int` | Maximum request time in milliseconds. `None` uses the runtime default. |
| `retryCount` | `Option Int` | Number of retry attempts for transient failures. `None` means no retries are requested. |
| `bearerToken` | `Option Text` | Adds an `Authorization: Bearer <token>` header when present. |
| `strictStatus` | `Option Bool` | When `Some True`, non-2xx responses are treated as errors instead of normal results. |

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text A</code> | Sends a `GET` request and decodes the response body into the expected type `A`. |
| **post** url body<br><code>Url -> Text -> Effect Text A</code> | Sends a `POST` request with a text body and decodes the response into `A`. |
| **fetch** request<br><code>Request -> Effect Text A</code> | Sends a custom REST request and decodes the response into the expected type `A`. |

## Choosing between `rest` and `http`

Choose `aivi.rest` when:

- the remote service behaves like a conventional REST API
- you want the result decoded directly into a value
- you want built-in support for bearer auth, retries, or stricter status handling

Choose `aivi.net.http` when:

- you need to inspect raw headers and response bodies yourself
- you want lower-level control over how requests are built and handled
- the remote service does not fit the usual REST pattern
