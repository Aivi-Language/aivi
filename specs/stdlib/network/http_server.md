# HTTP Server Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.httpServer"} -->
The `HttpServer` domain lets an AIVI program receive HTTP requests and send HTTP responses. Use it to build APIs, local tools, webhooks, or services that need to keep running and respond to network traffic.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.httpServer</div>

<<< ../../snippets/from_md/stdlib/network/http_server/http_server_domain.aivi{aivi}

## What this module is for

`aivi.net.httpServer` provides a runtime-managed HTTP server with support for HTTP/1.1, HTTP/2, and optional WebSocket upgrades. The runtime handles the server lifecycle; your code focuses on describing how to answer each request. If you need to call another service rather than host one, see [`aivi.net.http`](./http.md) or [`aivi.rest`](./rest.md).

Because `listen` returns a [`Resource HttpError Server`](../../syntax/resources.md), the server shuts down automatically when the surrounding resource scope ends. That makes it a natural fit for services that need clean startup and cleanup.

## Typical workflow

1. Create a `ServerConfig` with an address to bind, written as `host:port` such as `127.0.0.1:8080`.
2. Pass a handler function to `listen`.
3. Inspect the incoming `Request`.
4. Return either:
   - `Http response` for a normal HTTP response, or
   - `Ws handler` to upgrade the connection to WebSocket mode.

<<< ../../snippets/from_md/stdlib/network/http_server/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/network/http_server/types.aivi{aivi}

`Server` and `WebSocket` are opaque runtime-managed handles:

| Type | Description |
| --- | --- |
| `Server` | A running server instance returned by `listen`. Pass it to `stop` when you want to shut it down explicitly. |
| `WebSocket` | An active WebSocket connection. You use it with `wsRecv`, `wsSend`, and `wsClose`. |

### Request and response records

The included snippet defines the data you will work with most often:

- `Request` describes the incoming HTTP request.
- `Response` describes the reply you want to send back.
- `ServerReply` tells the runtime whether to send a normal HTTP response or upgrade to WebSocket handling.

Important fields:

| Record | Field | Meaning |
| --- | --- | --- |
| `Request` | `method` | The request method, such as `GET` or `POST`. |
| `Request` | `path` | The request path and query string, such as `/health` or `/users/42?page=1`. |
| `Request` | `headers` | Request headers from the client. |
| `Request` | `body` | Raw request body bytes. |
| `Request` | `remoteAddr` | The client address, if available, as `Option Text`. Use it for logging or access control. |
| `Response` | `status` | The HTTP status code to return. |
| `Response` | `headers` | Response headers. |
| `Response` | `body` | A `ResponseBody` value. Bare `Text`, record literals, and raw `List Int` byte lists are all accepted in the field through expected-type coercions. |

### `ResponseBody`

`ResponseBody` lets handlers stay ergonomic without giving up exact byte control:

- `RawBytes (List Int)` preserves the old raw-byte form.
- `Plain Text` sends UTF-8 text as written.
- `Form (List Header)` percent-encodes name/value pairs as `application/x-www-form-urlencoded` content.
- `Json JsonValue` sends structured JSON data.

When the expected type is `ResponseBody`, the compiler inserts the convenient wrappers below:

- bare `List Int` → `RawBytes bytes`
- bare `Text` → `Plain text`
- bare record literal `{ ... }` → `Json (toJson { ... })`
- bare `JsonValue` → `Json value`

Runtime header rule: JSON responses automatically add `Content-Type: application/json` when you do not already provide a `Content-Type` header. Plain text and form responses keep your headers unchanged.

## Functions

| Function | Explanation |
| --- | --- |
| **listen** config handler<br><code>ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource HttpError Server</code> | Starts a server with `config` and uses `handler` to answer each request. The returned resource cleans up the server when the scope ends. |
| **stop** server<br><code>Server -> Effect HttpError Unit</code> | Stops a running server. You can use this for explicit shutdown, though resource cleanup often makes it unnecessary. |
| **wsRecv** socket<br><code>WebSocket -> Effect WsError WsMessage</code> | Waits for the next WebSocket message from the client. |
| **wsSend** socket message<br><code>WebSocket -> WsMessage -> Effect WsError Unit</code> | Sends a WebSocket message back to the client. |
| **wsClose** socket<br><code>WebSocket -> Effect WsError Unit</code> | Closes the WebSocket connection. |

## WebSocket handling

To accept a WebSocket connection, return `Ws ...` instead of `Http ...` from your request handler. The snippet below shows the socket-side loop; your main request handler still decides which route should return `Ws echoSocket`.

<<< ../../snippets/from_md/stdlib/network/http_server/block_02.aivi{aivi}


This is a good pattern for chat-style features, live updates, or custom protocols that need a long-lived two-way connection. A common shape is to match `request.path` and return `pure (Ws echoSocket)` only for the route that should upgrade.

## Errors

Both HTTP server operations and WebSocket operations use simple error records:

<<< ../../snippets/from_md/stdlib/network/http_server/block_01.aivi{aivi}


If the main request handler returns `Err { message }`, the runtime answers with HTTP 500 and uses `message` as the response body. If you want a friendlier fallback response, catch the error inside your handler and return `Http ...` yourself. Invalid request data and missing or failed WebSocket upgrades produce HTTP 400 responses.
