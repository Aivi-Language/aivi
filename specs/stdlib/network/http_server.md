# HTTP Server Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.httpServer"} -->
The `HttpServer` domain provides a scalable HTTP/1.1 + HTTP/2 server with optional WebSocket upgrades. The server is designed to run across multiple CPU cores.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.httpServer</div>

<<< ../../snippets/from_md/stdlib/network/http_server/http_server_domain.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/network/http_server/types.aivi{aivi}

`Server` and `WebSocket` are opaque handle types returned by the runtime:

| Type | Description |
| --- | --- |
| `Server` | An active HTTP server instance, returned by `listen` and consumed by `stop`. |
| `WebSocket` | An open WebSocket connection, passed to the `Ws` handler inside `ServerReply`. |

## Functions

| Function | Explanation |
| --- | --- |
| **listen** config handler<br><code>ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server</code> | Starts a server and yields a `Server` resource that stops on cleanup. |
| **stop** server<br><code>Server -> Effect HttpError Unit</code> | Stops a running server instance. |
| **wsRecv** socket<br><code>WebSocket -> Effect WsError WsMessage</code> | Receives the next WebSocket message. |
| **wsSend** socket message<br><code>WebSocket -> WsMessage -> Effect WsError Unit</code> | Sends a WebSocket message. |
| **wsClose** socket<br><code>WebSocket -> Effect WsError Unit</code> | Closes the WebSocket connection. |
