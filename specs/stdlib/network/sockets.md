# Sockets Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.sockets"} -->
The `Sockets` domain exposes low-level TCP and UDP networking. Use it when you need to build a custom protocol, keep a long-lived connection open, or work below the HTTP layer.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.sockets</div>

<<< ../../snippets/from_md/stdlib/network/sockets/sockets_domain.aivi{aivi}

## What this module is for

`aivi.net.sockets` gives you direct access to socket-based networking primitives. It is a better fit than HTTP when:

- you are implementing your own wire protocol
- you need a custom binary connection
- you want UDP datagrams instead of request/response HTTP traffic

If you are building a web API or consuming one, `aivi.net.http`, `aivi.net.https`, or `aivi.rest` will usually be easier to work with.

## Types

<<< ../../snippets/from_md/stdlib/network/sockets/types.aivi{aivi}

`Listener` and `Connection` are opaque handles managed by the runtime:

| Type | Description |
| --- | --- |
| `Listener` | A bound TCP listener created by `listen`. Use it with `accept` to wait for incoming connections. |
| `Connection` | An established TCP connection used with `send`, `recv`, and `close`. |

## TCP

TCP is the usual choice when you need a reliable ordered byte stream between two endpoints.

### Common workflow

On the server side:

1. Call `listen` to bind to an address.
2. Call `accept` to wait for a client.
3. Exchange bytes with `recv` and `send`.
4. Close the connection when finished.

On the client side:

1. Call `connect`.
2. Use `send` and `recv`.
3. Call `close` when done.

```aivi
use aivi.net.sockets

sendGreeting = address => do Effect {
  connection <- connect address
  _ <- send connection [72, 101, 108, 108, 111] // "Hello" in bytes
  reply <- recv connection
  _ <- close connection
  pure reply
}
```

| Function | Explanation |
| --- | --- |
| **listen** address<br><code>Address -> Resource SocketError Listener</code> | Binds a TCP listener to `address`. The listener is cleaned up with its resource scope. |
| **accept** listener<br><code>Listener -> Effect SocketError Connection</code> | Waits for an incoming TCP connection and returns it. |
| **connect** address<br><code>Address -> Effect SocketError Connection</code> | Opens a TCP connection to a remote address. |
| **send** connection bytes<br><code>Connection -> List Int -> Effect SocketError Unit</code> | Sends raw bytes over a TCP connection. |
| **recv** connection<br><code>Connection -> Effect SocketError (List Int)</code> | Receives raw bytes from a TCP connection. |
| **close** connection<br><code>Connection -> Effect SocketError Unit</code> | Closes a TCP connection. |

## UDP

UDP is useful when you want lightweight datagrams and can tolerate missing, duplicated, or out-of-order packets.

```aivi
use aivi.net.sockets

sendDatagram = socket => address =>
  sendTo socket address [1, 2, 3, 4]
```

| Function | Explanation |
| --- | --- |
| **bindUdp** address<br><code>Address -> Resource SocketError UdpSocket</code> | Binds a UDP socket to `address`. |
| **sendTo** socket address bytes<br><code>UdpSocket -> Address -> List Int -> Effect SocketError Unit</code> | Sends one UDP datagram to `address`. |
| **recvFrom** socket<br><code>UdpSocket -> Effect SocketError { from: Address, bytes: List Int }</code> | Receives one UDP datagram and reports both the sender address and the bytes received. |
| **closeUdp** socket<br><code>UdpSocket -> Effect SocketError Unit</code> | Closes a UDP socket. |

## TCP vs UDP

Choose TCP when you need:

- delivery guarantees
- ordered data
- a connection that stays open across many messages

Choose UDP when you need:

- lightweight one-shot datagrams
- low overhead
- a protocol that already handles loss or ordering itself

## Errors

Socket operations return `SocketError` when something goes wrong:

```aivi
SocketError = { message: Text }
```

Typical causes include bind failures, connection failures, or reading from a closed socket.
