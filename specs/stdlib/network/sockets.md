# Sockets Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.sockets"} -->
The `Sockets` domain exposes low-level TCP/UDP sockets for custom protocols and long-lived connections.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.sockets</div>

<<< ../../snippets/from_md/stdlib/network/sockets/sockets_domain.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/network/sockets/types.aivi{aivi}

`Listener` and `Connection` are opaque handle types managed by the runtime:

| Type | Description |
| --- | --- |
| `Listener` | A bound TCP listener created by `listen`; passed to `accept` to wait for connections. |
| `Connection` | An established TCP connection; used with `send`, `recv`, and `close`. |

## TCP

| Function | Explanation |
| --- | --- |
| **listen** address<br><code>Address -> Resource Listener</code> | Creates a TCP listener bound to `address`. |
| **accept** listener<br><code>Listener -> Effect SocketError Connection</code> | Waits for and returns an incoming TCP connection. |
| **connect** address<br><code>Address -> Effect SocketError Connection</code> | Opens a TCP connection to `address`. |
| **send** connection bytes<br><code>Connection -> List Int -> Effect SocketError Unit</code> | Sends raw bytes to the remote endpoint. |
| **recv** connection<br><code>Connection -> Effect SocketError (List Int)</code> | Receives raw bytes from the remote endpoint. |
| **close** connection<br><code>Connection -> Effect SocketError Unit</code> | Closes the TCP connection. |

## UDP

| Function | Explanation |
| --- | --- |
| **bindUdp** address<br><code>Address -> Resource SocketError UdpSocket</code> | Binds a UDP socket to `address`. |
| **sendTo** socket address bytes<br><code>UdpSocket -> Address -> List Int -> Effect SocketError Unit</code> | Sends one datagram to `address`. |
| **recvFrom** socket<br><code>UdpSocket -> Effect SocketError { from: Address, bytes: List Int }</code> | Receives one datagram and source address. |
| **closeUdp** socket<br><code>UdpSocket -> Effect SocketError Unit</code> | Closes a UDP socket. |

## Notes

- TCP is connection-oriented and ordered.
- UDP is datagram-oriented and does not guarantee delivery/order.
