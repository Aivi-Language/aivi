# HTTP Server Domain

The `HttpServer` domain provides a scalable HTTP/1.1 + HTTP/2 server with optional WebSocket upgrades. The server is designed to run across multiple CPU cores.

```aivi
use aivi.std.network.http_server
```

## Types

```aivi
type Header = { name: Text, value: Text }

type Request = {
  method: Text,
  path: Text,
  headers: List Header,
  body: List Int,
  remote_addr: Option Text
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

### `listen`

```aivi
listen : ServerConfig -> (Request -> Effect HttpError ServerReply) -> Resource Server
```

Starts a server and yields a `Server` resource. The resource will stop the server on cleanup.

### `stop`

```aivi
stop : Server -> Effect HttpError Unit
```

Stops a running server.

### `ws_recv`

```aivi
ws_recv : WebSocket -> Effect WsError WsMessage
```

Receives the next WebSocket message.

### `ws_send`

```aivi
ws_send : WebSocket -> WsMessage -> Effect WsError Unit
```

Sends a WebSocket message.

### `ws_close`

```aivi
ws_close : WebSocket -> Effect WsError Unit
```

Closes the WebSocket connection.
