# Streams Domain

The `Streams` domain provides stream-oriented utilities for processing inbound and outbound data without loading everything into memory.

```aivi
use aivi.net.streams
```

## Types

```aivi
type StreamError = { message: Text }
```

## Functions

| Function | Explanation |
| --- | --- |
| **fromSocket** connection<br><pre><code>`Connection -> Stream (List Int)`</code></pre> | Creates a stream of byte chunks from the connection. |
| **toSocket** connection stream<br><pre><code>`Connection -> Stream (List Int) -> Effect StreamError Unit`</code></pre> | Writes byte chunks from `stream` to the connection. |
| **chunks** size stream<br><pre><code>`Int -> Stream (List Int) -> Stream (List Int)`</code></pre> | Rechunks a byte stream into fixed-size blocks of `size`. |
