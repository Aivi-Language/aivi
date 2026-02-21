# Sockets Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.sockets"} -->
The `Sockets` domain exposes low-level TCP/UDP sockets for custom protocols and long-lived connections.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.sockets</div>

<<< ../../snippets/from_md/05_stdlib/03_network/04_sockets/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/03_network/04_sockets/block_02.aivi{aivi}

## TCP

| Function | Explanation |
| --- | --- |
| **listen** address<br><pre><code>`Address -> Resource Listener`</code></pre> | Creates a TCP listener bound to `address`. |
| **accept** listener<br><pre><code>`Listener -> Effect SocketError Connection`</code></pre> | Waits for and returns an incoming TCP connection. |
| **connect** address<br><pre><code>`Address -> Effect SocketError Connection`</code></pre> | Opens a TCP connection to `address`. |
| **send** connection bytes<br><pre><code>`Connection -> List Int -> Effect SocketError Unit`</code></pre> | Sends raw bytes to the remote endpoint. |
| **recv** connection<br><pre><code>`Connection -> Effect SocketError (List Int)`</code></pre> | Receives raw bytes from the remote endpoint. |
| **close** connection<br><pre><code>`Connection -> Effect SocketError Unit`</code></pre> | Closes the TCP connection. |
