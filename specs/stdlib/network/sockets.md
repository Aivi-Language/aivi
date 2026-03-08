# Sockets Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.sockets"} -->
The `Sockets` domain exposes low-level TCP networking. Use it when you need to build a custom protocol, keep a long-lived connection open, or work below the HTTP layer.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.sockets</div>

<<< ../../snippets/from_md/stdlib/network/sockets/sockets_domain.aivi{aivi}

## What this module is for

`aivi.net.sockets` gives you direct access to TCP socket primitives. It is a better fit than HTTP when:

- you are implementing your own wire protocol
- you need a long-lived byte-stream connection
- you want direct control over raw bytes and connection lifecycle

If you are building a web API or consuming one, [`aivi.net.http`](./http.md), [`aivi.net.https`](./http.md#https-domain), or [`aivi.rest`](./rest.md) will usually be easier to work with.

If your protocol naturally works in chunks, pair this module with [`aivi.net.streams`](./streams.md) instead of writing every `recv` loop by hand.

## Capabilities

Socket operations use the `network` capability family:

| Operation | Capability |
| --- | --- |
| `listen` | `network.socket.listen` (or the broader `network`) |
| `connect` | `network.socket.connect` (or the broader `network`) |
| `accept`, `send`, `recv`, `close` | `network` |

## Types

<<< ../../snippets/from_md/stdlib/network/sockets/types.aivi{aivi}

The module exposes two ordinary records plus two opaque runtime-managed handles:

| Type | Description |
| --- | --- |
| `Address` | TCP endpoint record `{ host: Text, port: Int }`. `port` must fit in `0..65535` at runtime. |
| `SocketError` | Error record `{ message: Text }` returned when a socket operation fails. |
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

`listen` is resource-scoped, so the listener is cleaned up automatically when its `Resource` scope ends. `Connection` values are not resource-scoped, so every accepted or connected socket should be closed explicitly.

### Client example

<<< ../../snippets/from_md/stdlib/network/sockets/block_01.aivi{aivi}

### Functions

| Function | Explanation |
| --- | --- |
| **listen** address<br><code>Address -> Resource SocketError Listener</code> | Binds a TCP listener to `address`. The listener is cleaned up with its resource scope. |
| **accept** listener<br><code>Listener -> Effect SocketError Connection</code> | Waits for an incoming TCP connection and returns it. |
| **connect** address<br><code>Address -> Effect SocketError Connection</code> | Opens a TCP connection to a remote address. |
| **send** connection bytes<br><code>Connection -> List Int -> Effect SocketError Unit</code> | Sends raw bytes over a TCP connection. Each `Int` must be in the byte range `0..255`. |
| **recv** connection<br><code>Connection -> Effect SocketError (List Int)</code> | Receives the next available chunk of raw bytes from a TCP connection. An empty list means the peer closed the connection cleanly. |
| **close** connection<br><code>Connection -> Effect SocketError Unit</code> | Closes a TCP connection. |

## Practical guidance

- Use `listen` inside a `Resource` scope so listeners always shut down cleanly.
- Close every `Connection` you open or accept, even when the surrounding listener is resource-scoped.
- Reach for [`aivi.net.streams`](./streams.md) when you want chunked processing pipelines on top of a `Connection`.
- Reach for [`aivi.net.http`](./http.md) or [`aivi.rest`](./rest.md) when the remote protocol is already HTTP-based.

## Errors

Socket operations return `SocketError` when something goes wrong:

```aivi
SocketError = { message: Text }
```

`SocketError` is intentionally small: surface `message` directly for logs or wrap it in a more specific application error.

Typical causes include bind failures, connection failures, broken pipes, and operations on closed sockets.
