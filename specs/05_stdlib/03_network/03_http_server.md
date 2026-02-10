# HTTP Server Domain

The `HttpServer` domain provides a scalable HTTP/1.1 + HTTP/2 server with optional WebSocket upgrades. The server is designed to run across multiple CPU cores.

```aivi
use aivi.net.http_server
```

## Types

```aivi
type Header = { name: Text, value: Text }

type Request = {
  method: Text,
  path: Text,
  headers: List Header,
  body: List Int,
  remoteAddr: Option Text
}

type Response = {
  status: Int,
  headers: List Header,
  body: List Int
}

type ServerConfig = {
  address: Text
}

type HttpError = { message: Text }
type WsError = { message: Text }

type WsMessage
  = TextMsg Text
  | BinaryMsg (List Int)
  | Ping
  | Pong
  | Close

type ServerReply
  = Http Response
  | Ws (WebSocket -> Effect WsError Unit)
```

## Functions

| Function | Explanation |
| --- | --- |
| **listen** config handler<br><pre><code>`ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server`</code></pre> | Starts a server and yields a `Server` resource that stops on cleanup. |
| **stop** server<br><pre><code>`Server -> Effect HttpError Unit`</code></pre> | Stops a running server instance. |
| **wsRecv** socket<br><pre><code>`WebSocket -> Effect WsError WsMessage`</code></pre> | Receives the next WebSocket message. |
| **wsSend** socket message<br><pre><code>`WebSocket -> WsMessage -> Effect WsError Unit`</code></pre> | Sends a WebSocket message. |
| **wsClose** socket<br><pre><code>`WebSocket -> Effect WsError Unit`</code></pre> | Closes the WebSocket connection. |
