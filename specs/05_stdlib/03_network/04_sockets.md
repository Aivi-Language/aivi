# Sockets Domain

The `Sockets` domain exposes low-level TCP/UDP sockets for custom protocols and long-lived connections.

```aivi
use aivi.std.network.sockets
```

## Types

```aivi
type Address = { host: Text, port: Int }
type SocketError = { message: Text }
```

## TCP

### `listen`

```aivi
listen : Address -> Resource Listener
```

Creates a TCP listener.

### `accept`

```aivi
accept : Listener -> Effect SocketError Connection
```

Accepts an incoming connection.

### `connect`

```aivi
connect : Address -> Effect SocketError Connection
```

Connects to a remote TCP endpoint.

### `send`

```aivi
send : Connection -> List Int -> Effect SocketError Unit
```

Sends bytes to the remote endpoint.

### `recv`

```aivi
recv : Connection -> Effect SocketError (List Int)
```

Receives bytes from the remote endpoint.

### `close`

```aivi
close : Connection -> Effect SocketError Unit
```

Closes a connection.
