# REST Module

<!-- quick-info: {"kind":"module","name":"aivi.rest"} -->
The `rest` module is a higher-level HTTP client for JSON-based APIs. It is designed for the common case where you want to call a REST endpoint, decode the response into a value, and let the module handle details like bearer tokens, timeouts, retries, and status checks.
<!-- /quick-info -->
<div class="import-badge">use aivi.rest</div>

## What this module is for

Use `aivi.rest` when you are talking to a conventional REST API and want a friendlier interface than raw HTTP requests. In everyday API work, “REST” usually means resource-style HTTP endpoints that send and receive JSON.

Compared to [`aivi.net.http`](./http.md), this module adds:

- automatic response decoding into the type you expect,
- optional bearer-token authentication,
- optional request timeouts,
- optional retry counts for transient failures,
- optional strict handling of non-2xx status codes.

If you need full control over raw HTTP responses, use [`aivi.net.http`](./http.md) instead.

## Start here

A useful mental model is:

- [`aivi.net.http`](./http.md) = raw request/response control,
- `aivi.rest` = HTTP plus decoding, auth helpers, retry/timeouts, and optional strict status handling.

Choose the smallest entry point that fits:

| If you need to... | Start with... |
| --- | --- |
| fetch and decode JSON with defaults | `get` |
| send one simple text body and decode the response | `post` |
| configure timeouts, retries, bearer auth, or strict status rules | `fetch` |
| inspect raw headers and bodies yourself | [`aivi.net.http`](./http.md) |

## Typical example

This is the style of code `aivi.rest` is meant for:

```aivi
use aivi.rest

loadUser : Url -> Effect Text User
loadUser = userUrl => get userUrl
```

The expected result type (`User` here) tells the module what to decode from the response body.

For more control, build a `Request` value step by step:

```aivi
use aivi.rest

savePost : Url -> Text -> Effect Text SavedPost
savePost = postUrl => accessToken => do Effect {
  requestOptions = {
    method: "POST"
    url: postUrl
    headers: [{ name: "Accept", value: "application/json" }]
    body: Some (Plain "{\"title\":\"Hello\"}")
    timeoutMs: Some 5000
    retryCount: Some 2
    bearerToken: Some accessToken
    strictStatus: Some True
  }

  savedPost <- fetch requestOptions
  pure savedPost
}
```

Read the extra fields as policy choices:

- `timeoutMs` says how long you are willing to wait,
- `retryCount` says how many transient failures should be retried,
- `bearerToken` adds `Authorization: Bearer ...`,
- `strictStatus` turns non-2xx responses into errors.

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

- the remote service behaves like a conventional JSON API,
- you want the result decoded directly into a value,
- you want built-in support for bearer auth, retries, or stricter status handling,
- your main question is “what typed value did I get back?” rather than “what exact HTTP response came back?”.

Choose [`aivi.net.http`](./http.md) when:

- you need to inspect raw headers and response bodies yourself,
- you want lower-level control over how requests are built and handled,
- the remote service does not fit the usual REST pattern,
- you are debugging transport or protocol details.
