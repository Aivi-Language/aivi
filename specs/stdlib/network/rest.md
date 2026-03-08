# REST Module

<!-- quick-info: {"kind":"module","name":"aivi.rest"} -->
The `rest` module is a higher-level HTTP client for JSON-based APIs. It is designed for the common case where you want to call a REST endpoint, decode the response into a value, and let the module handle details like bearer tokens, timeouts, retries, and status checks.
<!-- /quick-info -->
<div class="import-badge">use aivi.rest</div>

This page covers the immediate convenience functions from `use aivi.rest`. If you want reusable `Source` declarations such as `rest.get { ... }` that you later pass to `load`, see [REST / HTTP Sources](../../syntax/external_sources/rest_http.md).

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

<<< ../../snippets/from_md/stdlib/network/rest/block_01.aivi{aivi}


The expected result type (`User` here) tells the module what to decode from the response body.

These helpers already perform `load` for you. Use `attempt (get userUrl)`, `attempt (post ...)`, or `attempt (fetch ...)` when you want to catch transport, decode, or strict-status failures explicitly.

For more control, build a `Request` value step by step:

<<< ../../snippets/from_md/stdlib/network/rest/block_02.aivi{aivi}


Read the extra fields as policy choices:

- `timeoutMs` says how long you are willing to wait,
- `retryCount` says how many transient failures should be retried,
- `bearerToken` adds `Authorization: Bearer ...`,
- `strictStatus` turns non-2xx responses into errors.

## How decoding and failures work

- `get`, `post`, and `fetch` are convenience wrappers over the lower-level REST source APIs. They return an `Effect` directly instead of a reusable `Source`.
- The surrounding result type drives response decoding. In `loadUser : Url -> Effect Text User`, the final `User` tells the loader what shape to expect from the response body.
- Transport failures always fail the effect. Decode failures also fail the effect, and `strictStatus: Some True` upgrades non-2xx HTTP responses into failures too.
- If you need reusable source declarations, schema-first source definitions, or composition with other source policies, use the REST source forms documented in [REST / HTTP Sources](../../syntax/external_sources/rest_http.md).

## Types

### `Header`

A single HTTP header as a name/value pair.

<<< ../../snippets/from_md/stdlib/network/rest/block_02.aivi{aivi}


### `Body`

`Body` describes what you send to the server.

<<< ../../snippets/from_md/stdlib/network/rest/block_03.aivi{aivi}


- `Plain Text` sends text exactly as written. Use this when you already have a JSON string or another raw payload.
- `Form (List Header)` sends form fields as `{ name, value }` pairs.

Unlike [`aivi.net.http`](./http.md), the convenience `aivi.rest` module does not expose a `Json` request-body variant today. If you want the JSON-body helper from the lower-level HTTP layer, use [`aivi.net.http`](./http.md); otherwise send JSON text with `Plain` or `post` and set any headers you need.

### `Request`

`Request` gathers the options you can set for a REST call.

<<< ../../snippets/from_md/stdlib/network/rest/block_04.aivi{aivi}


| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | The HTTP method, such as `GET`, `POST`, `PUT`, `PATCH`, or `DELETE`. |
| `url` | `Url` | The endpoint to call. |
| `headers` | `List Header` | Extra HTTP headers to send with the request. |
| `body` | `Option Body` | Optional request body. Use `Plain` for raw text or `Form` for form fields represented as `{ name, value }` pairs. |
| `timeoutMs` | `Option Int` | Maximum request time in milliseconds. `None` uses the runtime default. |
| `retryCount` | `Option Int` | Number of retry attempts for transient failures. `None` means no retries are requested. |
| `bearerToken` | `Option Text` | Adds an `Authorization: Bearer <token>` header when present. |
| `strictStatus` | `Option Bool` | When `Some True`, non-2xx responses are treated as errors instead of normal results. |

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text A</code> | Sends a `GET` request and decodes the response body into the expected type `A`. Use `attempt` if you want to recover from network or decode failures. |
| **post** url body<br><code>Url -> Text -> Effect Text A</code> | Sends a `POST` request with a text body and decodes the response into `A`. Reach for `fetch` when you also need headers, bearer auth, or other request options. |
| **fetch** request<br><code>Request -> Effect Text A</code> | Sends a custom REST request and decodes the response into the expected type `A`. This is the entry point for timeouts, retries, bearer auth, and strict status handling. |

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
