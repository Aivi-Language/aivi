# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain gives your program the basic building blocks for making HTTP requests. Use it when you want direct control over request methods, headers, and bodies while still working with a small, predictable API.
<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## What this module is for

`aivi.net.http` is the low-level HTTP client for AIVI programs. It is a good fit when you want to:

- fetch data from an API with `GET`,
- send form or JSON data with `POST`,
- build a fully custom request with headers, methods, and optional bodies,
- inspect the raw status code, headers, and text body yourself.

If you want a higher-level client for conventional JSON APIs, automatic bearer-token support, and built-in decoding into your target type, look at `aivi.rest`.

## Mental model

`aivi.net.http` is the “show me the raw envelope” tool:

- you build or inspect methods, headers, and bodies yourself,
- you decide what the response status means,
- you decide how to decode the body.

## HTTP vs REST in one minute

HTTP is the wire protocol: requests, methods, status codes, headers, and bodies. REST is a common style of API built on top of HTTP, usually with JSON payloads and predictable resource-oriented endpoints.

A practical chooser is:

- use **`aivi.net.http`** when you want to work with the raw request/response envelope,
- use **`aivi.rest`** when you mainly want “call endpoint, decode JSON, handle auth/timeouts/retries”.

## Start here

Choose the smallest entry point that matches the job:

| If you need to... | Start with... |
| --- | --- |
| fetch a URL with default behavior | `get` |
| send a simple text body with `POST` | `post` |
| set headers, choose a method, or send a structured body | `fetch` |
| call a JSON API and decode directly into a value | [`aivi.rest`](./rest.md) |

## Capabilities

`get`, `post`, and `fetch` perform network I/O when executed. The `rest` helper module covers the same runtime surface.

## Quick start

This example fetches a page and checks whether the request succeeded:

<<< ../../snippets/from_md/stdlib/network/http/block_01.aivi{aivi}


For requests that need a custom method, custom headers, or an explicit body, use `fetch`:

<<< ../../snippets/from_md/stdlib/network/http/block_02.aivi{aivi}

For JSON payloads, pass `Some (Json ...)` directly or rely on the `Body` coercions described below.


## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text (Result Error Response)</code> | Sends a `GET` request to `url`. Use this for read-only HTTP calls such as loading a page or fetching JSON as text. |
| **post** url body<br><code>Url -> Text -> Effect Text (Result Error Response)</code> | Sends a `POST` request with a text body. This is a convenient helper when you do not need custom headers or methods. |
| **fetch** request<br><code>Request -> Effect Text (Result Error Response)</code> | Sends a request built from a full `Request` record. Use this when you need custom methods, headers, or body handling. |

## Types

### `Header`

A single HTTP header as a name/value pair.

<<< ../../snippets/from_md/stdlib/network/http/block_01.aivi{aivi}


Typical examples:

<<< ../../snippets/from_md/stdlib/network/http/block_02.aivi{aivi}


### `Body`

`Body` describes what you send to the server.

<<< ../../snippets/from_md/stdlib/network/http/block_03.aivi{aivi}


Think of the variants like this:

- `Plain Text` sends raw text exactly as written.
- `Form (List Header)` reuses the `{ name, value }` shape from `Header` and percent-encodes the pairs as an `application/x-www-form-urlencoded` body.
- `Json JsonValue` sends structured JSON data.

When the expected type is `Body`, a plain record literal is automatically coerced to `Json (toJson record)`. See [expected-type coercions](../../syntax/types/expected_type_coercions.md#body-coercions) for the full rule. In practice, that means you can stay focused on the data you want to send:

<<< ../../snippets/from_md/stdlib/network/http/block_04.aivi{aivi}


Header rule: the `Json` variant automatically adds `Content-Type: application/json` when the request does not already define a `Content-Type` header. If you use `Form`, set `Content-Type` yourself when the server expects it.

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/block_05.aivi{aivi}


`Request` is the full request envelope. A useful way to read it is:

- `method` says what kind of action you want,
- `url` says where to send it,
- `headers` add metadata,
- `body` carries optional content.

| Field | Type | Explanation |
| --- | --- | --- |
| `method` | `Text` | The HTTP method, such as `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, or `HEAD`. |
| `url` | `Url` | The destination URL. |
| `headers` | `List Header` | Extra request headers to send with the request. |
| `body` | `Option Body` | An optional request body. Use `None` for requests without a body. |

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/block_06.aivi{aivi}


`Response` contains three things:

- `status`: the numeric HTTP status code, such as `200` or `404`,
- `headers`: the response headers returned by the server,
- `body`: the response body as text.

This makes it easy to inspect the status code first and then decide how to handle the body.

### `Error`

Returned in the `Err` branch when a request cannot be completed.

<<< ../../snippets/from_md/stdlib/network/http/block_07.aivi{aivi}


In practice, this is the value you inspect when:

- a connection cannot be opened,
- the remote server is unavailable,
- TLS or transport setup fails before a normal response is returned.

## Choosing between `http` and `rest`

Choose `aivi.net.http` when:

- you need raw response metadata,
- you want to decide yourself how to decode or inspect the body,
- the remote service does not behave like a typical JSON REST API,
- you are debugging at the protocol level.

Choose [`aivi.rest`](./rest.md) when:

- the service is a conventional JSON API,
- you want to decode directly into a typed value,
- bearer auth, retries, timeouts, or strict status rules are part of the normal call path.

---

## HTTPS Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.https"} -->
`aivi.net.https` mirrors `aivi.net.http` but always uses TLS. TLS (Transport Layer Security) is the standard encryption layer behind `https://` URLs. Choose it when you need encrypted connections, which is the normal choice for production APIs and any request carrying private data.
<!-- /quick-info -->
<div class="import-badge">use aivi.net.https</div>

`aivi.net.https` exports the same entry points (`get`, `post`, `fetch`) and the same request/response workflow as `aivi.net.http`, but it only accepts `https://` URLs and secures the connection with TLS. The `Header`, `Request`, `Response`, and `Error` shapes match `aivi.net.http`; its current `Body` type exposes `Plain` and `Form`.

### Common use

If an API gives you an `https://...` URL, import `aivi.net.https` and use it the same way you would use `aivi.net.http` for `GET`, text-body, and form-encoded requests:

<<< ../../snippets/from_md/stdlib/network/http/block_08.aivi{aivi}


### Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><code>Url -> Effect Text (Result Error Response)</code> | Sends a secure `GET` request over TLS. |
| **post** url body<br><code>Url -> Text -> Effect Text (Result Error Response)</code> | Sends a secure `POST` request with a text body. |
| **fetch** request<br><code>Request -> Effect Text (Result Error Response)</code> | Sends a fully custom secure request over TLS. |

### Types

The data types are:

<<< ../../snippets/from_md/stdlib/network/http/block_09.aivi{aivi}

