# HTTP Server Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.httpServer"} -->
The `HttpServer` domain provides a scalable HTTP/1.1 + HTTP/2 server with optional WebSocket upgrades. The server is designed to run across multiple CPU cores.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.httpServer</div>

<<< ../../snippets/from_md/stdlib/network/http_server/http_server_domain.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/network/http_server/types.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **listen** config handler<br><pre><code>`ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server`</code></pre> | Starts a server and yields a `Server` resource that stops on cleanup. |
| **stop** server<br><pre><code>`Server -> Effect HttpError Unit`</code></pre> | Stops a running server instance. |
| **wsRecv** socket<br><pre><code>`WebSocket -> Effect WsError WsMessage`</code></pre> | Receives the next WebSocket message. |
| **wsSend** socket message<br><pre><code>`WebSocket -> WsMessage -> Effect WsError Unit`</code></pre> | Sends a WebSocket message. |
| **wsClose** socket<br><pre><code>`WebSocket -> Effect WsError Unit`</code></pre> | Closes the WebSocket connection. |

## Routing Helpers

| Function | Explanation |
| --- | --- |
| **route** method path handler<br><pre><code>`Text -> Text -> (Request -> Effect HttpError ServerReply) -> Route`</code></pre> | Declares one route matcher and handler. |
| **router** routes<br><pre><code>`List Route -> (Request -> Effect HttpError ServerReply)`</code></pre> | Combines route declarations into a handler. |
| **mount** prefix handler<br><pre><code>`Text -> (Request -> Effect HttpError ServerReply) -> (Request -> Effect HttpError ServerReply)`</code></pre> | Rewrites paths for nested handlers. |

## Middleware

| Function | Explanation |
| --- | --- |
| **use** middleware handler<br><pre><code>`Middleware -> Handler -> Handler`</code></pre> | Applies middleware to a handler. |
| **compose** middlewares<br><pre><code>`List Middleware -> Middleware`</code></pre> | Combines middleware stack in declaration order. |
| **recover** handler<br><pre><code>`Handler -> Handler`</code></pre> | Converts uncaught handler failures into HTTP error replies. |
